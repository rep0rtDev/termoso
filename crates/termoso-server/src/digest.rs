//! Team activity digest: a team admin can ask to receive the team's activity
//! log by e-mail once a day or once a week. It is opt-in per admin, contains
//! only what that admin can already read in the activity log (routing
//! metadata — never entity payloads, which are ciphertext), and is not sent
//! at all for a quiet period. The software reports *to* the team, not on it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Days, NaiveDate, TimeDelta, Utc};
use sqlx::PgPool;
use termoso_proto::team::{
    DigestCadence, DigestSubscription, SendDigestRequest, SendDigestResponse, UpdateDigestRequest,
};
use uuid::Uuid;

use crate::error::{ApiResult, Error};
use crate::extract::{Auth, Json as Body};
use crate::mail::Mailer;
use crate::ratelimit;
use crate::routes::teams;
use crate::state::AppState;
use crate::users;

/// UTC hour at which a finished period is mailed.
pub const SEND_HOUR: u32 = 7;
/// How often the scheduler looks for due digests.
const TICK: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// Events listed one per line; the rest are summarised as a count.
const MAX_LINES: usize = 60;
/// Hard cap on rows read per digest (counts stay exact up to here).
const MAX_EVENTS: i64 = 2000;

/// Half-open time range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

fn length(cadence: DigestCadence) -> TimeDelta {
    match cadence {
        DigestCadence::Daily => TimeDelta::days(1),
        DigestCadence::Weekly => TimeDelta::weeks(1),
    }
}

fn midnight(d: NaiveDate) -> DateTime<Utc> {
    d.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc()
}

/// The most recent complete period whose digest is due at `now`: the last UTC
/// day (daily) or the last Monday-to-Sunday week (weekly), once `SEND_HOUR`
/// has passed after it ended.
pub fn due_period(cadence: DigestCadence, now: DateTime<Utc>) -> Period {
    let today = now.date_naive();
    let anchor = match cadence {
        DigestCadence::Daily => today,
        DigestCadence::Weekly => today
            .checked_sub_days(Days::new(u64::from(
                chrono::Datelike::weekday(&today).num_days_from_monday(),
            )))
            .expect("date in range"),
    };
    let mut end = midnight(anchor);
    if now < end + TimeDelta::hours(i64::from(SEND_HOUR)) {
        end -= length(cadence);
    }
    Period {
        start: end - length(cadence),
        end,
    }
}

/// Background loop: every few minutes, mail every digest that became due.
pub async fn run(state: AppState) {
    let Some(mailer) = state.mailer.clone() else {
        return;
    };
    let web_url = state.cfg.web_url().to_string();
    loop {
        match send_due(&state.db, &mailer, &web_url, Utc::now()).await {
            Ok(0) => {}
            Ok(n) => tracing::info!(count = n, "sent team activity digests"),
            Err(e) => tracing::warn!(error = %e, "team activity digests failed"),
        }
        tokio::time::sleep(TICK).await;
    }
}

type DueRow = (Uuid, Uuid, String, String, String, DateTime<Utc>);

/// Mail every subscription whose period ended and is not yet covered by
/// `last_sent_at`; returns how many e-mails went out. Safe to run on several
/// instances at once: each row is claimed with a conditional update first.
pub async fn send_due(
    db: &PgPool,
    mailer: &Mailer,
    web_url: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<usize> {
    // Subscriptions of people who are no longer admins of the team go away;
    // the digest is an admin view of the log.
    sqlx::query(
        "DELETE FROM team_digests d
         WHERE NOT EXISTS (SELECT 1 FROM team_members m
                           WHERE m.team_id = d.team_id AND m.user_id = d.user_id
                             AND m.role IN ('owner', 'admin'))",
    )
    .execute(db)
    .await?;

    let daily = due_period(DigestCadence::Daily, now);
    let weekly = due_period(DigestCadence::Weekly, now);
    let rows: Vec<DueRow> = sqlx::query_as(
        "SELECT d.team_id, d.user_id, d.cadence, u.email, t.name, d.last_sent_at
         FROM team_digests d
         JOIN users u ON u.id = d.user_id
         JOIN teams t ON t.id = d.team_id
         WHERE u.email_verified
           AND ((d.cadence = 'daily' AND d.last_sent_at < $1)
             OR (d.cadence = 'weekly' AND d.last_sent_at < $2))",
    )
    .bind(daily.end)
    .bind(weekly.end)
    .fetch_all(db)
    .await?;

    let mut sent = 0;
    for (team_id, user_id, cadence, email, team_name, previous) in rows {
        let Some(cadence) = DigestCadence::parse(&cadence) else {
            continue;
        };
        let period = match cadence {
            DigestCadence::Daily => daily,
            DigestCadence::Weekly => weekly,
        };
        let claimed = sqlx::query(
            "UPDATE team_digests SET last_sent_at = $3
             WHERE team_id = $1 AND user_id = $2 AND last_sent_at < $3",
        )
        .bind(team_id)
        .bind(user_id)
        .bind(period.end)
        .execute(db)
        .await?
        .rows_affected();
        if claimed == 0 {
            continue;
        }
        let events = load(db, team_id, period).await?;
        if events.is_empty() {
            continue;
        }
        let (subject, text) = render(&team_name, team_id, cadence, period, &events, web_url);
        match mailer.send(&email, &subject, &text).await {
            Ok(()) => {
                sent += 1;
                metrics::counter!("termoso_team_digests_sent_total", "cadence" => cadence.as_str())
                    .increment(1);
            }
            Err(e) => {
                tracing::warn!(error = %e, %team_id, "could not send activity digest");
                // Give the period back so the next tick retries it.
                sqlx::query(
                    "UPDATE team_digests SET last_sent_at = $3
                     WHERE team_id = $1 AND user_id = $2 AND last_sent_at = $4",
                )
                .bind(team_id)
                .bind(user_id)
                .bind(previous)
                .bind(period.end)
                .execute(db)
                .await?;
            }
        }
    }
    Ok(sent)
}

/// One audit row with the names needed to describe it.
pub struct DigestEvent {
    pub action: String,
    pub actor_email: Option<String>,
    pub actor_name: Option<String>,
    pub actor_id: Option<Uuid>,
    pub target_user: Option<Uuid>,
    pub target_email: Option<String>,
    pub vault_name: Option<String>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

type EventRow = (
    String,
    Option<String>,
    Option<String>,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    Option<String>,
    serde_json::Value,
    DateTime<Utc>,
);

/// Team events inside `period`, oldest first, at most `MAX_EVENTS + 1` so the
/// caller can tell that the cap was hit.
pub async fn load(db: &PgPool, team_id: Uuid, period: Period) -> anyhow::Result<Vec<DigestEvent>> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT e.action, a.email, a.display_name, e.actor_id, e.target_user, t.email, v.name,
                e.details, e.created_at
         FROM team_audit_events e
         LEFT JOIN users a ON a.id = e.actor_id
         LEFT JOIN users t ON t.id = e.target_user
         LEFT JOIN vaults v ON v.id = e.vault_id
         WHERE e.team_id = $1 AND e.created_at >= $2 AND e.created_at < $3
         ORDER BY e.id LIMIT $4",
    )
    .bind(team_id)
    .bind(period.start)
    .bind(period.end)
    .bind(MAX_EVENTS + 1)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                action,
                actor_email,
                actor_name,
                actor_id,
                target_user,
                target_email,
                vault_name,
                details,
                created_at,
            )| DigestEvent {
                action,
                actor_email,
                actor_name,
                actor_id,
                target_user,
                target_email,
                vault_name,
                details,
                created_at,
            },
        )
        .collect())
}

/// Coarse groups for the summary block, by action prefix.
const GROUPS: &[(&str, &str)] = &[
    ("team.", "Team settings"),
    ("member.", "Members"),
    ("invite.", "Invitations"),
    ("vault.", "Vaults & access"),
    ("entity.", "Shared data"),
    ("multiplayer.", "Multiplayer"),
];

/// Subject and plain-text body. `events` is what [`load`] returned.
pub fn render(
    team_name: &str,
    team_id: Uuid,
    cadence: DigestCadence,
    period: Period,
    events: &[DigestEvent],
    web_url: &str,
) -> (String, String) {
    let last_day = (period.end - TimeDelta::days(1)).date_naive();
    let when = match cadence {
        DigestCadence::Daily => last_day.format("%A, %-d %B %Y").to_string(),
        DigestCadence::Weekly => {
            let first = period.start.date_naive();
            if first.format("%B %Y").to_string() == last_day.format("%B %Y").to_string() {
                format!("{}–{}", first.format("%-d"), last_day.format("%-d %B %Y"))
            } else {
                format!(
                    "{} – {}",
                    first.format("%-d %B"),
                    last_day.format("%-d %B %Y")
                )
            }
        }
    };
    let subject = format!("{team_name}: activity for {when}");

    let capped = events.len() > MAX_EVENTS as usize;
    let events = &events[..events.len().min(MAX_EVENTS as usize)];
    let people: BTreeSet<&str> = events
        .iter()
        .filter_map(|e| e.actor_email.as_deref())
        .collect();

    let mut text = String::new();
    let _ = writeln!(text, "Activity in \"{team_name}\" — {when}");
    text.push('\n');
    let _ = writeln!(
        text,
        "{}{} by {} {}.",
        if capped { "Over " } else { "" },
        plural(events.len(), "event", "events"),
        people.len(),
        if people.len() == 1 {
            "person"
        } else {
            "people"
        }
    );
    text.push('\n');
    text.push_str("By type:\n");
    for (prefix, label) in GROUPS {
        let n = events
            .iter()
            .filter(|e| e.action.starts_with(prefix))
            .count();
        if n > 0 {
            let _ = writeln!(text, "  {label}: {n}");
        }
    }
    text.push('\n');
    text.push_str("Timeline (UTC):\n");
    let stamp = match cadence {
        DigestCadence::Daily => "%H:%M",
        DigestCadence::Weekly => "%a %-d %b %H:%M",
    };
    // Display names are free text and may collide; add the address when they do.
    let mut names: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for e in events {
        if let (Some(n), Some(m)) = (e.actor_name.as_deref(), e.actor_email.as_deref()) {
            names.entry(n).or_default().insert(m);
        }
    }
    for e in events.iter().take(MAX_LINES) {
        let mut line = describe(e);
        if let (Some(n), Some(m)) = (e.actor_name.as_deref(), e.actor_email.as_deref())
            && names.get(n).is_some_and(|set| set.len() > 1)
        {
            line.actor = format!("{n} ({m})");
        }
        let _ = write!(
            text,
            "  {}  {} {}",
            e.created_at.format(stamp),
            line.actor,
            line.text
        );
        if let Some(v) = &line.vault {
            let _ = write!(text, " (in \"{v}\")");
        }
        if let Some(m) = &line.meta {
            let _ = write!(text, " — {m}");
        }
        text.push('\n');
    }
    if events.len() > MAX_LINES {
        let _ = writeln!(
            text,
            "  … {} not shown.",
            plural(events.len() - MAX_LINES, "more event", "more events")
        );
    }
    text.push('\n');
    let _ = writeln!(
        text,
        "You receive this because you turned on the activity digest for \"{team_name}\".\n\
         Change or stop it in the team settings: {}/team/{team_id}",
        web_url.trim_end_matches('/')
    );
    (subject, text)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

struct Line {
    actor: String,
    text: String,
    vault: Option<String>,
    meta: Option<String>,
}

fn str_of<'a>(d: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    d.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn num_of(d: &serde_json::Value, key: &str) -> Option<u64> {
    d.get(key).and_then(|v| v.as_u64())
}

fn bool_of(d: &serde_json::Value, key: &str) -> Option<bool> {
    d.get(key).and_then(|v| v.as_bool())
}

fn len_of(d: &serde_json::Value, key: &str) -> usize {
    d.get(key).and_then(|v| v.as_array()).map_or(0, Vec::len)
}

fn role_text(r: Option<&str>) -> String {
    match r {
        Some("viewer") => "can view".into(),
        Some("editor") => "can edit".into(),
        Some("manager") => "can manage".into(),
        Some("owner") => "Owner".into(),
        Some("admin") => "Admin".into(),
        Some("member") => "Member".into(),
        Some(other) => other.into(),
        None => String::new(),
    }
}

fn kind_label(kind: Option<&str>, count: u64) -> String {
    let (one, many) = match kind {
        Some("host") => ("a host", "hosts"),
        Some("group") => ("a group", "groups"),
        Some("ssh_key") => ("an SSH key", "SSH keys"),
        Some("ssh_certificate") => ("a certificate", "certificates"),
        Some("identity") => ("an identity", "identities"),
        Some("known_host") => ("a known host", "known hosts"),
        Some("snippet") => ("a snippet", "snippets"),
        Some("snippet_package") => ("a snippet package", "snippet packages"),
        Some("host_snippet") => ("a snippet target", "snippet targets"),
        Some("pf_rule") => ("a port forwarding rule", "port forwarding rules"),
        Some("proxy") => ("a proxy", "proxies"),
        Some("host_chain") => ("a host chain", "host chains"),
        Some("tag") => ("a tag", "tags"),
        Some("tag_host") => ("a tag link", "tag links"),
        Some("ssh_config") => ("an SSH config", "SSH configs"),
        Some("telnet_config") => ("a Telnet config", "Telnet configs"),
        Some("serial_config") => ("a serial config", "serial configs"),
        Some("port_knocking") => ("a port knocking", "port knockings"),
        Some("workspace") => ("a workspace", "workspaces"),
        Some("workspace_template") => ("a workspace template", "workspace templates"),
        Some("log_bookmark") => ("a log bookmark", "log bookmarks"),
        Some("cloud_import") => ("a cloud import", "cloud imports"),
        Some(k) => {
            return if count == 1 {
                format!("a {k}")
            } else {
                format!("{count} {k}s")
            };
        }
        None => ("an item", "items"),
    };
    if count == 1 {
        one.to_string()
    } else {
        format!("{count} {many}")
    }
}

/// Same sentences the desktop activity log shows (`apps/desktop/src/team/activity.ts`).
fn describe(e: &DigestEvent) -> Line {
    let d = &e.details;
    let actor = e
        .actor_name
        .clone()
        .or_else(|| e.actor_email.clone())
        .unwrap_or_else(|| "Someone".into());
    let target = e
        .target_email
        .clone()
        .or_else(|| str_of(d, "email").map(str::to_string))
        .unwrap_or_else(|| "a member".into());
    let is_self = e.target_user.is_some() && e.target_user == e.actor_id;
    let vault = e
        .vault_name
        .clone()
        .or_else(|| str_of(d, "name").map(str::to_string));
    let role = str_of(d, "role");
    let prev = str_of(d, "previous_role");
    let name = str_of(d, "name");

    let mut meta = None;
    let text = match e.action.as_str() {
        "team.created" => match name {
            Some(n) => format!("created the team \"{n}\""),
            None => "created the team".into(),
        },
        "team.renamed" => format!("renamed the team to \"{}\"", name.unwrap_or("…")),
        "team.settings" => match (
            bool_of(d, "multiplayer_enabled"),
            bool_of(d, "require_mfa"),
            bool_of(d, "presence_enabled"),
        ) {
            (Some(on), _, _) => format!("{} Multiplayer", if on { "enabled" } else { "disabled" }),
            (_, Some(on), _) => format!(
                "{} 2FA for the team",
                if on { "required" } else { "stopped requiring" }
            ),
            (_, _, Some(on)) => format!("{} presence", if on { "enabled" } else { "disabled" }),
            _ => "changed team settings".into(),
        },
        "member.role" => {
            if prev.is_some() {
                meta = Some(format!("was {}", role_text(prev)));
            }
            format!(
                "changed {} role to {}",
                if is_self {
                    "their own".to_string()
                } else {
                    format!("{target}'s")
                },
                role_text(role)
            )
        }
        "member.removed" => {
            if prev.is_some() {
                meta = Some(format!("was {}", role_text(prev)));
            } else if str_of(d, "account") == Some("converted") {
                meta = Some("account converted to individual".into());
            }
            format!("removed {target} from the team")
        }
        "member.account_deleted" => {
            if role.is_some() {
                meta = Some(format!("was {}", role_text(role)));
            }
            format!("deleted {target}'s account")
        }
        "member.left" => {
            if str_of(d, "account") == Some("converted") {
                meta = Some("account converted to individual".into());
            }
            "left the team".into()
        }
        "invite.created" => {
            let n = len_of(d, "vault_ids");
            if n > 0 {
                meta = Some(format!("with access to {}", plural(n, "vault", "vaults")));
            }
            let mut t = format!("invited {}", str_of(d, "email").unwrap_or("someone"));
            if role.is_some() {
                let _ = write!(t, " as {}", role_text(role));
            }
            t
        }
        "invite.revoked" => format!(
            "revoked the invitation for {}",
            str_of(d, "email").unwrap_or("someone")
        ),
        "invite.accepted" => {
            let mut t = String::from("joined the team");
            if role.is_some() {
                let _ = write!(t, " as {}", role_text(role));
            }
            t
        }
        "vault.created" => {
            let n = len_of(d, "members");
            if n > 0 {
                meta = Some(format!("shared with {}", plural(n, "member", "members")));
            }
            format!("created the vault \"{}\"", vault.as_deref().unwrap_or("…"))
        }
        "vault.renamed" => format!("renamed a vault to \"{}\"", name.unwrap_or("…")),
        "vault.deleted" => {
            if let Some(n) = num_of(d, "members") {
                meta = Some(format!("{n} member(s) lost access"));
            }
            format!(
                "deleted the vault \"{}\"",
                name.or(vault.as_deref()).unwrap_or("…")
            )
        }
        "vault.access_granted" => format!("gave {target} access ({})", role_text(role)),
        "vault.access_changed" => {
            if prev.is_some() {
                meta = Some(format!("was {}", role_text(prev)));
            }
            format!("changed {target}'s access to {}", role_text(role))
        }
        "vault.access_revoked" => {
            if prev.is_some() {
                meta = Some(format!("was {}", role_text(prev)));
            }
            if bool_of(d, "self") == Some(true) {
                "left the vault".into()
            } else {
                format!("removed {target}'s access")
            }
        }
        "vault.key_rotated" => {
            let dropped = len_of(d, "access_dropped");
            let mut m = format!(
                "re-sealed for {}",
                plural(len_of(d, "resealed_for"), "member", "members")
            );
            if dropped > 0 {
                let _ = write!(m, ", {dropped} lost access");
            }
            meta = Some(m);
            match num_of(d, "key_version") {
                Some(v) => format!("rotated the vault key (v{v})"),
                None => "rotated the vault key".into(),
            }
        }
        "entity.created" => format!(
            "added {}",
            kind_label(str_of(d, "kind"), num_of(d, "count").unwrap_or(1))
        ),
        "entity.updated" => format!(
            "updated {}",
            kind_label(str_of(d, "kind"), num_of(d, "count").unwrap_or(1))
        ),
        "entity.deleted" => format!(
            "removed {}",
            kind_label(str_of(d, "kind"), num_of(d, "count").unwrap_or(1))
        ),
        "multiplayer.started" => "started a multiplayer session".into(),
        "multiplayer.joined" => "joined a multiplayer session".into(),
        "multiplayer.stopped" => "stopped a multiplayer session".into(),
        other => other.replacen('.', ": ", 1).replace('_', " "),
    };
    // Vault names only matter for vault-scoped events; "name" in team events
    // is the team.
    let vault = if e.action.starts_with("team.") {
        None
    } else {
        e.vault_name.clone()
    };
    Line {
        actor,
        text,
        vault,
        meta,
    }
}

async fn subscription(db: &PgPool, team_id: Uuid, user_id: Uuid) -> ApiResult<DigestSubscription> {
    let row: Option<(String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT cadence, last_sent_at FROM team_digests WHERE team_id = $1 AND user_id = $2",
    )
    .bind(team_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some((c, at)) => DigestSubscription {
            cadence: DigestCadence::parse(&c),
            last_sent_at: Some(at),
        },
        None => DigestSubscription {
            cadence: None,
            last_sent_at: None,
        },
    })
}

/// The caller's own digest subscription for this team (admins only).
#[utoipa::path(get, path = "/api/v1/teams/{id}/digest", tag = "teams",
    params(("id" = Uuid, Path)), responses((status = 200, body = DigestSubscription)))]
pub async fn get(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<DigestSubscription>> {
    teams::require_admin(&state, id, auth.user_id()).await?;
    Ok(Json(subscription(&state.db, id, auth.user_id()).await?))
}

async fn require_mailbox(state: &AppState, user_id: Uuid) -> ApiResult<String> {
    if state.mailer.is_none() {
        return Err(Error::feature_disabled("Email"));
    }
    let u = users::by_id(&state.db, user_id).await?;
    if !u.email_verified {
        return Err(Error::email_unverified());
    }
    Ok(u.email)
}

/// Subscribe to (or unsubscribe from) the digest. Only for the caller: an
/// admin cannot sign anyone else up.
#[utoipa::path(put, path = "/api/v1/teams/{id}/digest", tag = "teams",
    params(("id" = Uuid, Path)), request_body = UpdateDigestRequest,
    responses((status = 200, body = DigestSubscription)))]
pub async fn put(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<UpdateDigestRequest>,
) -> ApiResult<Json<DigestSubscription>> {
    teams::require_admin(&state, id, auth.user_id()).await?;
    match req.cadence {
        Some(cadence) => {
            require_mailbox(&state, auth.user_id()).await?;
            sqlx::query(
                "INSERT INTO team_digests (team_id, user_id, cadence) VALUES ($1, $2, $3)
                 ON CONFLICT (team_id, user_id) DO UPDATE SET cadence = EXCLUDED.cadence",
            )
            .bind(id)
            .bind(auth.user_id())
            .bind(cadence.as_str())
            .execute(&state.db)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM team_digests WHERE team_id = $1 AND user_id = $2")
                .bind(id)
                .bind(auth.user_id())
                .execute(&state.db)
                .await?;
        }
    }
    Ok(Json(subscription(&state.db, id, auth.user_id()).await?))
}

/// Mail the caller the digest for the most recent day/week right now. Nothing
/// goes out for a quiet period.
#[utoipa::path(post, path = "/api/v1/teams/{id}/digest/send", tag = "teams",
    params(("id" = Uuid, Path)), request_body = SendDigestRequest,
    responses((status = 200, body = SendDigestResponse)))]
pub async fn send_now(
    State(state): State<AppState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Body(req): Body<SendDigestRequest>,
) -> ApiResult<Json<SendDigestResponse>> {
    teams::require_admin(&state, id, auth.user_id()).await?;
    let email = require_mailbox(&state, auth.user_id()).await?;
    let Some(mailer) = &state.mailer else {
        return Err(Error::feature_disabled("Email"));
    };
    let cadence = match req.cadence {
        Some(c) => c,
        None => subscription(&state.db, id, auth.user_id())
            .await?
            .cadence
            .unwrap_or(DigestCadence::Daily),
    };
    let now = Utc::now();
    let period = Period {
        start: now - length(cadence),
        end: now,
    };
    let events = load(&state.db, id, period).await?;
    if events.is_empty() {
        return Ok(Json(SendDigestResponse {
            events: 0,
            sent: false,
        }));
    }
    ratelimit::check(&state, ratelimit::EMAIL, &email).await?;
    let team_name = teams::team_name(&state.db, id).await?;
    let (subject, text) = render(
        &team_name,
        id,
        cadence,
        period,
        &events,
        state.cfg.web_url(),
    );
    mailer.send(&email, &subject, &text).await?;
    metrics::counter!("termoso_team_digests_sent_total", "cadence" => "manual").increment(1);
    Ok(Json(SendDigestResponse {
        events: events.len().min(MAX_EVENTS as usize) as u32,
        sent: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).expect("rfc3339").to_utc()
    }

    #[test]
    fn daily_period_waits_for_send_hour() {
        // Before 07:00 UTC the day that just ended is not due yet.
        let p = due_period(DigestCadence::Daily, at("2026-09-15T06:59:00Z"));
        assert_eq!(p.start, at("2026-09-13T00:00:00Z"));
        assert_eq!(p.end, at("2026-09-14T00:00:00Z"));
        let p = due_period(DigestCadence::Daily, at("2026-09-15T07:00:00Z"));
        assert_eq!(p.start, at("2026-09-14T00:00:00Z"));
        assert_eq!(p.end, at("2026-09-15T00:00:00Z"));
    }

    #[test]
    fn weekly_period_is_monday_to_sunday() {
        // 2026-09-14 is a Monday.
        let p = due_period(DigestCadence::Weekly, at("2026-09-14T06:00:00Z"));
        assert_eq!(p.start, at("2026-08-31T00:00:00Z"));
        assert_eq!(p.end, at("2026-09-07T00:00:00Z"));
        let p = due_period(DigestCadence::Weekly, at("2026-09-14T07:00:00Z"));
        assert_eq!(p.start, at("2026-09-07T00:00:00Z"));
        assert_eq!(p.end, at("2026-09-14T00:00:00Z"));
        // Mid-week still points at last week.
        let p = due_period(DigestCadence::Weekly, at("2026-09-17T12:00:00Z"));
        assert_eq!(p.end, at("2026-09-14T00:00:00Z"));
    }

    fn ev(action: &str, details: serde_json::Value) -> DigestEvent {
        DigestEvent {
            action: action.into(),
            actor_email: Some("alice@example.com".into()),
            actor_name: Some("Alice".into()),
            actor_id: Some(Uuid::nil()),
            target_user: None,
            target_email: Some("bob@example.com".into()),
            vault_name: Some("Staging".into()),
            details,
            created_at: at("2026-09-14T09:12:00Z"),
        }
    }

    #[test]
    fn renders_summary_timeline_and_footer() {
        let team = Uuid::nil();
        let period = Period {
            start: at("2026-09-14T00:00:00Z"),
            end: at("2026-09-15T00:00:00Z"),
        };
        let events = vec![
            ev(
                "invite.created",
                serde_json::json!({ "email": "bob@example.com", "role": "admin", "vault_ids": ["x"] }),
            ),
            ev(
                "entity.created",
                serde_json::json!({ "kind": "host", "count": 3 }),
            ),
            ev(
                "vault.access_granted",
                serde_json::json!({ "role": "editor" }),
            ),
        ];
        let (subject, text) = render(
            "Ops",
            team,
            DigestCadence::Daily,
            period,
            &events,
            "https://cabinet.example/",
        );
        assert_eq!(subject, "Ops: activity for Monday, 14 September 2026");
        assert!(text.contains("3 events by 1 person."));
        assert!(text.contains("  Invitations: 1\n"));
        assert!(text.contains("  Shared data: 1\n"));
        assert!(text.contains(
            "09:12  Alice invited bob@example.com as Admin (in \"Staging\") — with access to 1 vault"
        ));
        assert!(text.contains("Alice added 3 hosts (in \"Staging\")"));
        assert!(text.contains("Alice gave bob@example.com access (can edit)"));
        assert!(text.contains("https://cabinet.example/team/00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn weekly_subject_and_truncation() {
        let period = Period {
            start: at("2026-08-31T00:00:00Z"),
            end: at("2026-09-07T00:00:00Z"),
        };
        let events: Vec<DigestEvent> = (0..(MAX_LINES + 5))
            .map(|_| ev("multiplayer.started", serde_json::json!({})))
            .collect();
        let (subject, text) = render(
            "Ops",
            Uuid::nil(),
            DigestCadence::Weekly,
            period,
            &events,
            "http://x",
        );
        assert_eq!(subject, "Ops: activity for 31 August – 6 September 2026");
        assert!(text.contains("… 5 more events not shown."));
        assert!(text.contains("Mon 14 Sep 09:12"));
    }
}
