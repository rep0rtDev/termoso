//! Profile pictures of the account and teammates, cached on disk by content
//! tag so a picture is downloaded once per change, not once per launch.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

use crate::error::Result;
use crate::state::AppState;

const DIR: &str = "avatars";

fn dir(state: &AppState) -> PathBuf {
    state.profile_dir.join(DIR)
}

fn cached(state: &AppState, user_id: Uuid, tag: &str) -> PathBuf {
    dir(state).join(format!("{user_id}-{tag}.webp"))
}

/// Drop older pictures of the same user once a new one is stored.
fn prune(state: &AppState, user_id: Uuid, keep: &str) {
    let Ok(entries) = std::fs::read_dir(dir(state)) else {
        return;
    };
    let prefix = format!("{user_id}-");
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name != keep {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The WebP bytes of picture `tag` of `user_id`, from the disk cache or the
/// server. `None` when the server no longer has it.
pub async fn user_avatar<R: Runtime>(
    app: &AppHandle<R>,
    user_id: Uuid,
    tag: String,
) -> Result<Option<Vec<u8>>> {
    if tag.is_empty() || !tag.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(None);
    }
    let state = app.state::<AppState>();
    let path = cached(&state, user_id, &tag);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(Some(bytes));
    }
    let api = crate::account::api(app).await?;
    let Some(bytes) = api.user_avatar(user_id).await? else {
        return Ok(None);
    };
    std::fs::create_dir_all(dir(&state))?;
    std::fs::write(&path, &bytes)?;
    prune(
        &state,
        user_id,
        &path.file_name().unwrap_or_default().to_string_lossy(),
    );
    Ok(Some(bytes))
}

/// Forget every cached picture (sign-out).
pub fn clear_cache(state: &AppState) {
    let _ = std::fs::remove_dir_all(dir(state));
}
