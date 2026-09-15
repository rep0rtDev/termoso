//! Team presence: who is connected to which team-vault host right now.

mod common;

use std::time::Duration;

use common::*;
use futures::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use termoso_crypto::keys::SymmetricKey;
use termoso_crypto::sealed;
use termoso_proto::account::{PresenceVisibilityRequest, UserProfile};
use termoso_proto::auth::AuthResponse;
use termoso_proto::team::{
    CreateInviteRequest, CreateTeamRequest, PresenceSession, Team, TeamPresence, TeamRole,
    UpdateTeamRequest,
};
use termoso_proto::vault::{CreateVaultRequest, Vault, VaultMemberUpsert, VaultRole};
use termoso_proto::ws::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

macro_rules! server {
    () => {
        match server().await {
            Some(s) => s,
            None => return,
        }
    };
}

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn send(ws: &mut Ws, msg: &ClientMessage) {
    ws.send(Message::Text(serde_json::to_string(msg).unwrap().into()))
        .await
        .unwrap();
}

/// Open the realtime socket, authenticate and swallow `Hello`.
async fn connect(s: &TestServer, token: &str) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(s.ws_url()).await.unwrap();
    send(
        &mut ws,
        &ClientMessage::Auth {
            token: token.into(),
        },
    )
    .await;
    match next(&mut ws).await {
        Some(ServerMessage::Hello { .. }) => ws,
        other => panic!("expected hello, got {other:?}"),
    }
}

async fn next(ws: &mut Ws) -> Option<ServerMessage> {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("ws timeout")?
            .ok()?;
        match msg {
            Message::Text(t) => return Some(serde_json::from_str(&t).unwrap()),
            Message::Close(_) => return None,
            _ => continue,
        }
    }
}

/// Wait for `PresenceChanged { team_id }`, skipping unrelated notifications.
async fn until_presence(ws: &mut Ws, team_id: Uuid) {
    for _ in 0..20 {
        match next(ws).await {
            Some(ServerMessage::PresenceChanged { team_id: t }) if t == team_id => return,
            Some(
                ServerMessage::PresenceChanged { .. }
                | ServerMessage::TeamsUpdated
                | ServerMessage::VaultsUpdated
                | ServerMessage::AccountUpdated
                | ServerMessage::Pong,
            ) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("no PresenceChanged for {team_id}");
}

/// Discard whatever notifications have piled up so the next wait is real.
async fn drain(ws: &mut Ws) {
    while let Ok(Some(Ok(_))) = tokio::time::timeout(Duration::from_millis(500), ws.next()).await {}
}

/// Assert that nothing (other than housekeeping) arrives within a short window.
async fn expect_quiet(ws: &mut Ws) {
    loop {
        match tokio::time::timeout(Duration::from_millis(700), ws.next()).await {
            Err(_) => return,
            Ok(Some(Ok(Message::Text(t)))) => {
                let m: ServerMessage = serde_json::from_str(&t).unwrap();
                assert!(
                    matches!(m, ServerMessage::Pong),
                    "expected silence, got {m:?}"
                );
            }
            Ok(Some(Ok(_))) => continue,
            Ok(other) => panic!("socket died: {other:?}"),
        }
    }
}

async fn presence(s: &TestServer, team_id: Uuid, token: &str) -> TeamPresence {
    s.json(
        Method::GET,
        &format!("/teams/{team_id}/presence"),
        Some(token),
        NOBODY,
    )
    .await
}

fn session(vault_id: Uuid, host_id: Uuid, protocol: &str) -> PresenceSession {
    PresenceSession {
        vault_id,
        host_id,
        protocol: protocol.into(),
        since: chrono::Utc::now(),
    }
}

async fn set_presence_enabled(s: &TestServer, team_id: Uuid, token: &str, on: bool) -> Team {
    s.json(
        Method::PATCH,
        &format!("/teams/{team_id}"),
        Some(token),
        Some(&UpdateTeamRequest {
            name: None,
            multiplayer_enabled: None,
            require_mfa: None,
            presence_enabled: Some(on),
        }),
    )
    .await
}

/// Invite `who` into the team as a member and accept.
async fn join(s: &TestServer, team: &Team, owner: &User, who: &User) {
    let invite: serde_json::Value = s
        .json(
            Method::POST,
            &format!("/teams/{}/invites", team.id),
            Some(owner.token()),
            Some(&CreateInviteRequest {
                email: who.email.clone(),
                role: TeamRole::Member,
                vault_ids: vec![],
            }),
        )
        .await;
    let token = invite["url"].as_str().unwrap().rsplit('/').next().unwrap();
    let _: Team = s
        .json(
            Method::POST,
            &format!("/invites/{token}/accept"),
            Some(who.token()),
            NOBODY,
        )
        .await;
}

async fn team_vault(s: &TestServer, team: &Team, owner: &User, members: &[&User]) -> Vault {
    let key = SymmetricKey::generate();
    let mut list = vec![VaultMemberUpsert {
        user_id: owner.id(),
        role: VaultRole::Manager,
        sealed_key: sealed::seal_vault_key(owner.keypair.public(), &key).unwrap(),
    }];
    for m in members {
        list.push(VaultMemberUpsert {
            user_id: m.id(),
            role: VaultRole::Editor,
            sealed_key: sealed::seal_vault_key(m.keypair.public(), &key).unwrap(),
        });
    }
    s.json(
        Method::POST,
        &format!("/teams/{}/vaults", team.id),
        Some(owner.token()),
        Some(&CreateVaultRequest {
            name: "Vault".into(),
            members: list,
        }),
    )
    .await
}

#[tokio::test]
async fn presence_end_to_end() {
    let s = server!();
    let owner = register(s, &unique_email("pr-owner"), "pw-owner-1234567").await;
    let bob = register(s, &unique_email("pr-bob"), "pw-bob-123456789").await;
    let carol = register(s, &unique_email("pr-carol"), "pw-carol-12345678").await;
    let outsider = register(s, &unique_email("pr-out"), "pw-outsider-1234").await;

    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest { name: "Ops".into() }),
        )
        .await;
    assert!(!team.presence_enabled, "off by default");
    join(s, &team, &owner, &bob).await;
    join(s, &team, &owner, &carol).await;
    // Shared: owner + bob. Private: owner only. Carol is in the team but in no vault.
    let shared = team_vault(s, &team, &owner, &[&bob]).await;
    let private = team_vault(s, &team, &owner, &[]).await;
    let personal_vault = {
        let v: termoso_proto::vault::VaultList = s
            .json(Method::GET, "/vaults", Some(owner.token()), NOBODY)
            .await;
        v.vaults
            .iter()
            .find(|v| v.kind == termoso_proto::vault::VaultKind::Personal)
            .expect("personal vault")
            .id
    };
    let shared_host = Uuid::new_v4();
    let private_host = Uuid::new_v4();
    let personal_host = Uuid::new_v4();
    let on_shared = session(shared.id, shared_host, "ssh");
    let on_private = session(private.id, private_host, "sftp");
    let on_personal = session(personal_vault, personal_host, "ssh");

    // Outsiders get nothing; members can't flip the team switch.
    s.expect_status(
        Method::GET,
        &format!("/teams/{}/presence", team.id),
        Some(outsider.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;
    s.expect_status(
        Method::PATCH,
        &format!("/teams/{}", team.id),
        Some(bob.token()),
        Some(&UpdateTeamRequest {
            name: None,
            multiplayer_enabled: None,
            require_mfa: None,
            presence_enabled: Some(true),
        }),
        StatusCode::FORBIDDEN,
    )
    .await;

    // While the team switch is off, reports are dropped and the snapshot says so.
    let mut owner_ws = connect(s, owner.token()).await;
    let mut bob_ws = connect(s, bob.token()).await;
    send(
        &mut owner_ws,
        &ClientMessage::Presence {
            sessions: vec![on_shared.clone()],
        },
    )
    .await;
    expect_quiet(&mut bob_ws).await;
    let p = presence(s, team.id, bob.token()).await;
    assert!(!p.enabled);
    assert!(p.entries.is_empty());

    // Admin turns it on → everyone is told, the device re-reports.
    let t = set_presence_enabled(s, team.id, owner.token(), true).await;
    assert!(t.presence_enabled);
    until_presence(&mut bob_ws, team.id).await;
    until_presence(&mut owner_ws, team.id).await;
    let p = presence(s, team.id, bob.token()).await;
    assert!(p.enabled);
    assert!(p.entries.is_empty(), "the earlier report was not stored");

    // Owner reports shared + private + personal. Bob sees only the shared one,
    // the owner sees shared + private; the personal session is never stored.
    let since = chrono::Utc::now();
    send(
        &mut owner_ws,
        &ClientMessage::Presence {
            sessions: vec![
                on_shared.clone(),
                on_private.clone(),
                on_personal.clone(),
                on_shared.clone(),
            ],
        },
    )
    .await;
    until_presence(&mut bob_ws, team.id).await;
    let p = presence(s, team.id, bob.token()).await;
    assert_eq!(p.entries.len(), 1);
    let e = &p.entries[0];
    assert_eq!(e.user_id, owner.id());
    assert_eq!(e.email, owner.email.to_lowercase());
    assert_eq!(e.device_id, owner.session.device_id);
    assert!(!e.device_name.is_empty());
    assert!(e.seen_at >= since);
    assert_eq!(
        e.sessions.len(),
        1,
        "duplicates collapse; private stays private"
    );
    assert_eq!(e.sessions[0].vault_id, shared.id);
    assert_eq!(e.sessions[0].host_id, shared_host);
    assert_eq!(e.sessions[0].protocol, "ssh");
    let mine = presence(s, team.id, owner.token()).await;
    assert_eq!(mine.entries.len(), 1);
    let hosts: Vec<Uuid> = mine.entries[0].sessions.iter().map(|x| x.host_id).collect();
    assert_eq!(hosts.len(), 2);
    assert!(hosts.contains(&shared_host) && hosts.contains(&private_host));
    assert!(!hosts.contains(&personal_host));
    // Carol is in the team but has no vault → nothing to see.
    let p = presence(s, team.id, carol.token()).await;
    assert!(p.enabled);
    assert!(p.entries.is_empty());

    // Heartbeat with the same list: no fan-out, but seen_at moves forward.
    let first_seen = e.seen_at;
    tokio::time::sleep(Duration::from_millis(20)).await;
    send(
        &mut owner_ws,
        &ClientMessage::Presence {
            sessions: vec![on_shared.clone(), on_private.clone()],
        },
    )
    .await;
    expect_quiet(&mut bob_ws).await;
    let p = presence(s, team.id, bob.token()).await;
    assert!(p.entries[0].seen_at > first_seen);

    // A second device of the owner shows up as its own entry.
    let AuthResponse::Authenticated(second) = login(s, &owner.email, &owner.password).await else {
        panic!("login")
    };
    let mut owner_ws2 = connect(s, &second.token).await;
    send(
        &mut owner_ws2,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "mosh")],
        },
    )
    .await;
    until_presence(&mut bob_ws, team.id).await;
    let p = presence(s, team.id, bob.token()).await;
    assert_eq!(p.entries.len(), 2);
    assert!(p.entries.iter().all(|e| e.user_id == owner.id()));
    assert!(p.entries.iter().any(|e| e.device_id == second.device_id));

    // Bob joins in; the owner sees him.
    drain(&mut owner_ws).await;
    send(
        &mut bob_ws,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "ssh")],
        },
    )
    .await;
    until_presence(&mut owner_ws, team.id).await;
    let p = presence(s, team.id, owner.token()).await;
    assert!(p.entries.iter().any(|e| e.user_id == bob.id()));

    // Bob hides himself: gone at once, and later reports are not stored.
    let profile: UserProfile = s
        .json(
            Method::PUT,
            "/account/presence",
            Some(bob.token()),
            Some(&PresenceVisibilityRequest { hidden: true }),
        )
        .await;
    assert!(profile.presence_hidden);
    until_presence(&mut owner_ws, team.id).await;
    let p = presence(s, team.id, owner.token()).await;
    assert!(p.entries.iter().all(|e| e.user_id != bob.id()));
    send(
        &mut bob_ws,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "ssh")],
        },
    )
    .await;
    expect_quiet(&mut owner_ws).await;
    assert!(
        presence(s, team.id, owner.token())
            .await
            .entries
            .iter()
            .all(|e| e.user_id != bob.id())
    );
    // Hidden users still see everyone else.
    assert!(!presence(s, team.id, bob.token()).await.entries.is_empty());
    let profile: UserProfile = s
        .json(
            Method::PUT,
            "/account/presence",
            Some(bob.token()),
            Some(&PresenceVisibilityRequest { hidden: false }),
        )
        .await;
    assert!(!profile.presence_hidden);
    send(
        &mut bob_ws,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "ssh")],
        },
    )
    .await;
    until_presence(&mut owner_ws, team.id).await;
    assert!(
        presence(s, team.id, owner.token())
            .await
            .entries
            .iter()
            .any(|e| e.user_id == bob.id())
    );

    // A device that stopped reporting drops out after the stale window.
    s.age_presence(team.id, owner.id(), second.device_id).await;
    let p = presence(s, team.id, bob.token()).await;
    assert!(p.entries.iter().all(|e| e.device_id != second.device_id));
    assert!(
        p.entries
            .iter()
            .any(|e| e.device_id == owner.session.device_id)
    );

    // An empty list clears the device; closing the socket does too.
    drain(&mut bob_ws).await;
    send(
        &mut owner_ws2,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "ssh")],
        },
    )
    .await;
    until_presence(&mut bob_ws, team.id).await;
    send(
        &mut owner_ws2,
        &ClientMessage::Presence { sessions: vec![] },
    )
    .await;
    until_presence(&mut bob_ws, team.id).await;
    assert!(
        presence(s, team.id, bob.token())
            .await
            .entries
            .iter()
            .all(|e| e.device_id != second.device_id)
    );
    owner_ws.close(None).await.unwrap();
    until_presence(&mut bob_ws, team.id).await;
    let p = presence(s, team.id, bob.token()).await;
    assert!(p.entries.iter().all(|e| e.user_id != owner.id()));
    assert_eq!(p.entries.len(), 1, "bob is still there");

    // Removing a member wipes what they reported.
    s.expect_status(
        Method::DELETE,
        &format!("/teams/{}/members/{}", team.id, bob.id()),
        Some(owner.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    assert!(presence(s, team.id, owner.token()).await.entries.is_empty());
    s.expect_status(
        Method::GET,
        &format!("/teams/{}/presence", team.id),
        Some(bob.token()),
        NOBODY,
        StatusCode::NOT_FOUND,
    )
    .await;

    // Turning the switch off forgets everything; on again starts empty.
    drain(&mut owner_ws2).await;
    send(
        &mut owner_ws2,
        &ClientMessage::Presence {
            sessions: vec![session(shared.id, shared_host, "ssh")],
        },
    )
    .await;
    until_presence(&mut owner_ws2, team.id).await;
    assert_eq!(presence(s, team.id, owner.token()).await.entries.len(), 1);
    set_presence_enabled(s, team.id, owner.token(), false).await;
    let p = presence(s, team.id, owner.token()).await;
    assert!(!p.enabled && p.entries.is_empty());
    set_presence_enabled(s, team.id, owner.token(), true).await;
    let p = presence(s, team.id, owner.token()).await;
    assert!(p.enabled && p.entries.is_empty());
}
