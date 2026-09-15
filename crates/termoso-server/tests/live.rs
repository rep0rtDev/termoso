//! Multiplayer relay: create / join / relay / control / stop, team flag.

mod common;

use std::time::Duration;

use common::*;
use futures::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use termoso_crypto::live::{self, Direction, FrameKind, LiveSecret};
use termoso_proto::live::*;
use termoso_proto::team::{CreateTeamRequest, Team, UpdateTeamRequest};
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

async fn connect(s: &TestServer, id: Uuid, token: &str, join: Option<String>) -> Ws {
    let url = format!("ws://{}/api/v1/live/{id}/ws", s.addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    ws.send(Message::Text(
        serde_json::to_string(&LiveClientMessage::Auth {
            token: token.into(),
            join_token: join,
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    ws
}

enum Frame {
    Text(LiveServerMessage),
    Bin(Vec<u8>),
    Closed,
}

async fn next(ws: &mut Ws) -> Frame {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("ws timeout");
        match msg {
            None => return Frame::Closed,
            Some(Err(_)) => return Frame::Closed,
            Some(Ok(Message::Text(t))) => return Frame::Text(serde_json::from_str(&t).unwrap()),
            Some(Ok(Message::Binary(b))) => return Frame::Bin(b.to_vec()),
            Some(Ok(Message::Close(_))) => return Frame::Closed,
            Some(Ok(_)) => continue,
        }
    }
}

async fn next_text(ws: &mut Ws) -> LiveServerMessage {
    match next(ws).await {
        Frame::Text(m) => m,
        Frame::Bin(_) => panic!("unexpected binary frame"),
        Frame::Closed => panic!("socket closed"),
    }
}

async fn next_bin(ws: &mut Ws) -> Vec<u8> {
    loop {
        match next(ws).await {
            Frame::Bin(b) => return b,
            Frame::Text(LiveServerMessage::Participants { .. }) => continue,
            Frame::Text(m) => panic!("unexpected text frame {m:?}"),
            Frame::Closed => panic!("socket closed"),
        }
    }
}

/// Wait for a participants update satisfying `pred`.
async fn until_participants(ws: &mut Ws, pred: impl Fn(&[LiveParticipant]) -> bool) {
    for _ in 0..10 {
        match next_text(ws).await {
            LiveServerMessage::Participants { participants } if pred(&participants) => return,
            LiveServerMessage::Participants { .. } | LiveServerMessage::Control { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    panic!("participants never matched");
}

#[tokio::test]
async fn multiplayer_relay_end_to_end() {
    let s = server!();
    let host = register(s, &unique_email("mp-host"), "pw-mp-host-1234567").await;
    let viewer = register(s, &unique_email("mp-view"), "pw-mp-view-1234567").await;

    // Host creates the session; the server learns only a hash of the join token.
    let secret = LiveSecret::generate();
    let created: LiveSession = s
        .json(
            Method::POST,
            "/live",
            Some(host.token()),
            Some(&CreateLiveSessionRequest {
                join_token: secret.join_token().unwrap(),
            }),
        )
        .await;
    assert_eq!(created.host_user_id, host.id());
    assert!(created.ended_at.is_none());
    let sid = created.id;
    let key = secret.stream_key(&sid.to_string()).unwrap();

    let listed: LiveSessionList = s
        .json(Method::GET, "/live", Some(host.token()), NOBODY)
        .await;
    assert!(listed.sessions.iter().any(|l| l.id == sid));

    // Wrong link → rejected.
    let mut bad = connect(s, sid, viewer.token(), Some("nope".repeat(12))).await;
    match next_text(&mut bad).await {
        LiveServerMessage::Error { code, .. } => assert_eq!(code, "forbidden"),
        other => panic!("expected error, got {other:?}"),
    }

    // Host connects (no join token needed).
    let mut hws = connect(s, sid, host.token(), None).await;
    match next_text(&mut hws).await {
        LiveServerMessage::Hello {
            is_host,
            participants,
            ..
        } => {
            assert!(is_host);
            assert_eq!(participants.len(), 1);
            assert!(participants[0].is_host && participants[0].can_write);
        }
        other => panic!("expected hello, got {other:?}"),
    }
    // Own presence echo.
    until_participants(&mut hws, |p| p.len() == 1).await;

    // Viewer joins with the link.
    let mut vws = connect(s, sid, viewer.token(), Some(secret.join_token().unwrap())).await;
    match next_text(&mut vws).await {
        LiveServerMessage::Hello {
            is_host,
            participants,
            ..
        } => {
            assert!(!is_host);
            assert_eq!(participants.len(), 2);
            let me = participants
                .iter()
                .find(|p| p.user_id == viewer.id())
                .unwrap();
            assert!(!me.can_write);
        }
        other => panic!("expected hello, got {other:?}"),
    }
    until_participants(&mut hws, |p| p.len() == 2).await;
    until_participants(&mut vws, |p| p.len() == 2).await;

    // Host output → viewer, encrypted end to end.
    let frame = live::seal_frame(
        &key,
        &sid.to_string(),
        Direction::FromHost,
        FrameKind::Output,
        b"$ ls\r\n",
    )
    .unwrap();
    hws.send(Message::Binary(frame.clone().into()))
        .await
        .unwrap();
    let got = next_bin(&mut vws).await;
    assert_eq!(got, frame);
    let (kind, pt) = live::open_frame(&key, &sid.to_string(), Direction::FromHost, &got).unwrap();
    assert_eq!(kind, FrameKind::Output);
    assert_eq!(pt, b"$ ls\r\n");

    // Viewer input is dropped while read-only…
    let input = live::seal_frame(
        &key,
        &sid.to_string(),
        Direction::FromViewer,
        FrameKind::Input,
        b"rm -rf\n",
    )
    .unwrap();
    vws.send(Message::Binary(input.clone().into()))
        .await
        .unwrap();
    // …then granted remote control by the host.
    hws.send(Message::Text(
        serde_json::to_string(&LiveClientMessage::Control {
            user_id: viewer.id(),
            enabled: true,
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    let mut granted = false;
    for _ in 0..4 {
        match next_text(&mut vws).await {
            LiveServerMessage::Control { can_write } => {
                granted = can_write;
                break;
            }
            LiveServerMessage::Participants { .. } => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(granted);
    until_participants(&mut hws, |p| {
        p.iter().any(|x| x.user_id == viewer.id() && x.can_write)
    })
    .await;

    let input2 = live::seal_frame(
        &key,
        &sid.to_string(),
        Direction::FromViewer,
        FrameKind::Input,
        b"echo hi\n",
    )
    .unwrap();
    vws.send(Message::Binary(input2.clone().into()))
        .await
        .unwrap();
    let got = next_bin(&mut hws).await;
    assert_eq!(
        got, input2,
        "the read-only frame must not have been relayed"
    );

    // Direct (catch-up) frame to one viewer.
    hws.send(Message::Text(
        serde_json::to_string(&LiveClientMessage::Direct {
            user_id: viewer.id(),
            data: {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(&frame)
            },
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    assert_eq!(next_bin(&mut vws).await, frame);

    // Host stops: viewer gets Ended and the link dies.
    s.expect_status(
        Method::POST,
        &format!("/live/{sid}/stop"),
        Some(viewer.token()),
        NOBODY,
        StatusCode::FORBIDDEN,
    )
    .await;
    s.expect_status(
        Method::POST,
        &format!("/live/{sid}/stop"),
        Some(host.token()),
        NOBODY,
        StatusCode::NO_CONTENT,
    )
    .await;
    let mut ended = false;
    for _ in 0..6 {
        match next(&mut vws).await {
            Frame::Text(LiveServerMessage::Ended) => {
                ended = true;
                break;
            }
            Frame::Text(_) => {}
            Frame::Bin(_) => {}
            Frame::Closed => break,
        }
    }
    assert!(ended, "viewer should be told the session ended");
    let mut late = connect(s, sid, viewer.token(), Some(secret.join_token().unwrap())).await;
    match next_text(&mut late).await {
        LiveServerMessage::Error { code, .. } => assert_eq!(code, "ended"),
        other => panic!("expected ended, got {other:?}"),
    }
    let listed: LiveSessionList = s
        .json(Method::GET, "/live", Some(host.token()), NOBODY)
        .await;
    assert!(listed.sessions.iter().all(|l| l.id != sid));
}

#[tokio::test]
async fn host_disconnect_ends_session() {
    let s = server!();
    let host = register(s, &unique_email("mp-host2"), "pw-mp-host-1234567").await;
    let viewer = register(s, &unique_email("mp-view2"), "pw-mp-view-1234567").await;
    let secret = LiveSecret::generate();
    let created: LiveSession = s
        .json(
            Method::POST,
            "/live",
            Some(host.token()),
            Some(&CreateLiveSessionRequest {
                join_token: secret.join_token().unwrap(),
            }),
        )
        .await;
    let sid = created.id;
    let mut hws = connect(s, sid, host.token(), None).await;
    assert!(matches!(
        next_text(&mut hws).await,
        LiveServerMessage::Hello { .. }
    ));
    let mut vws = connect(s, sid, viewer.token(), Some(secret.join_token().unwrap())).await;
    assert!(matches!(
        next_text(&mut vws).await,
        LiveServerMessage::Hello { .. }
    ));
    drop(hws);
    let mut ended = false;
    for _ in 0..6 {
        match next(&mut vws).await {
            Frame::Text(LiveServerMessage::Ended) | Frame::Closed => {
                ended = true;
                break;
            }
            _ => {}
        }
    }
    assert!(ended);
}

#[tokio::test]
async fn team_flag_disables_multiplayer() {
    let s = server!();
    let owner = register(s, &unique_email("mp-owner"), "pw-mp-owner-123456").await;
    let team: Team = s
        .json(
            Method::POST,
            "/teams",
            Some(owner.token()),
            Some(&CreateTeamRequest {
                name: "NoMP".into(),
            }),
        )
        .await;
    let _: Team = s
        .json(
            Method::PATCH,
            &format!("/teams/{}", team.id),
            Some(owner.token()),
            Some(&UpdateTeamRequest {
                name: None,
                multiplayer_enabled: Some(false),
                require_mfa: None,
                presence_enabled: None,
            }),
        )
        .await;
    let body = s
        .expect_status(
            Method::POST,
            "/live",
            Some(owner.token()),
            Some(&CreateLiveSessionRequest {
                join_token: LiveSecret::generate().join_token().unwrap(),
            }),
            StatusCode::FORBIDDEN,
        )
        .await;
    assert_eq!(body["code"], "multiplayer_disabled");

    // Re-enable → works again.
    let _: Team = s
        .json(
            Method::PATCH,
            &format!("/teams/{}", team.id),
            Some(owner.token()),
            Some(&UpdateTeamRequest {
                name: None,
                multiplayer_enabled: Some(true),
                require_mfa: None,
                presence_enabled: None,
            }),
        )
        .await;
    let _: LiveSession = s
        .json(
            Method::POST,
            "/live",
            Some(owner.token()),
            Some(&CreateLiveSessionRequest {
                join_token: LiveSecret::generate().join_token().unwrap(),
            }),
        )
        .await;
}
