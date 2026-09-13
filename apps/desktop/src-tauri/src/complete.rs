//! Data the terminal autocomplete needs from the session side: directory
//! listings for path completion and stored passwords typed into the
//! terminal on the user's explicit request.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tauri::State;
use termoso_core::model::Identity;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

const LIST_TIMEOUT: Duration = Duration::from_secs(4);
const MAX_ENTRIES: usize = 2000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub dir: bool,
}

/// Entries of `path` as seen by the session: relative paths resolve against
/// `cwd` (reported by the shell integration), `~` against the user's home.
/// SSH sessions run `ls` on a separate channel, so the interactive shell and
/// its history never see it; local sessions read the directory directly.
#[tauri::command]
pub async fn terminal_list_dir(
    state: State<'_, AppState>,
    id: Uuid,
    cwd: Option<String>,
    path: String,
) -> Result<Vec<DirEntry>> {
    if path.contains('\0') || path.len() > 4096 {
        return Err(DesktopError::invalid("bad path"));
    }
    let info = state.sessions.info(id)?;
    match info.protocol.as_str() {
        "local" => list_local(cwd.as_deref(), &path),
        "ssh" => {
            let client = state.sessions.client(id)?;
            let cmd = remote_ls_command(cwd.as_deref(), &path);
            let out = tokio::time::timeout(LIST_TIMEOUT, client.exec(&cmd, None))
                .await
                .map_err(|_| DesktopError::invalid("listing timed out"))??;
            Ok(parse_ls(&out.stdout_str()))
        }
        _ => Ok(Vec::new()),
    }
}

fn list_local(cwd: Option<&str>, path: &str) -> Result<Vec<DirEntry>> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let expanded: PathBuf = if let Some(rest) = path.strip_prefix('~') {
        let Some(home) = home else {
            return Ok(Vec::new());
        };
        home.join(rest.trim_start_matches(['/', '\\']))
    } else if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        let base = cwd.map(PathBuf::from).or(home).unwrap_or_default();
        base.join(path)
    };
    let Ok(read) = std::fs::read_dir(&expanded) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<DirEntry> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let dir = e
                .file_type()
                .map(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()))
                .unwrap_or(false);
            Some(DirEntry { name, dir })
        })
        .take(MAX_ENTRIES)
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// One `ls` that works in GNU coreutils, BusyBox and the BSDs: `-1` one per
/// line, `-a` hidden files too, `-p` slash after directories, `-L` so a
/// symlink to a directory gets the slash.
fn remote_ls_command(cwd: Option<&str>, path: &str) -> String {
    let (base, target) = match path.strip_prefix('~') {
        Some(rest) => (
            Some("\"$HOME\"".to_string()),
            format!(".{}", if rest.is_empty() { "/" } else { rest }),
        ),
        None if path.starts_with('/') => (None, path.to_string()),
        None => (cwd.map(shell_quote), path.to_string()),
    };
    let cd = base
        .map(|b| format!("cd {b} 2>/dev/null && "))
        .unwrap_or_default();
    format!(
        "{cd}LC_ALL=C ls -1apL -- {} 2>/dev/null | head -n {}",
        shell_quote(&target),
        MAX_ENTRIES + 2
    )
}

fn parse_ls(out: &str) -> Vec<DirEntry> {
    out.lines()
        .filter(|l| !l.is_empty() && *l != "./" && *l != "../")
        .map(|l| match l.strip_suffix('/') {
            Some(name) => DirEntry {
                name: name.to_string(),
                dir: true,
            },
            None => DirEntry {
                name: l.to_string(),
                dir: false,
            },
        })
        .take(MAX_ENTRIES)
        .collect()
}

/// Type a stored password into the session, followed by Enter. Only ever
/// called from an explicit click / key in the autocomplete popup; the value
/// goes straight from the vault into the PTY and never crosses the IPC
/// boundary. `identity_id` = `None` means the identity the host connected
/// with.
#[tauri::command]
pub async fn terminal_insert_password(
    state: State<'_, AppState>,
    id: Uuid,
    identity_id: Option<Uuid>,
) -> Result<()> {
    let info = state.sessions.info(id)?;
    let password: Zeroizing<String> = match identity_id {
        Some(iid) => state
            .store
            .require::<Identity>(iid)?
            .data
            .password
            .filter(|p| !p.is_empty())
            .map(Zeroizing::new)
            .ok_or_else(|| DesktopError::invalid("identity has no password"))?,
        None => {
            let host_id = info
                .host_id
                .ok_or_else(|| DesktopError::invalid("session has no saved host"))?;
            state
                .store
                .resolve_host(host_id)?
                .identity
                .and_then(|i| i.data.password)
                .filter(|p| !p.is_empty())
                .map(Zeroizing::new)
                .ok_or_else(|| DesktopError::invalid("host has no stored password"))?
        }
    };
    let term = state.sessions.terminal(id)?;
    let mut line = Zeroizing::new(Vec::with_capacity(password.len() + 1));
    line.extend_from_slice(password.as_bytes());
    line.push(b'\r');
    term.write(&line).await?;
    Ok(())
}

/// Label of the identity a saved-host session authenticated with (so the UI
/// can offer "password of <label>" without seeing the password).
#[tauri::command]
pub fn terminal_host_identity(state: State<'_, AppState>, id: Uuid) -> Result<Option<String>> {
    let info = state.sessions.info(id)?;
    let Some(host_id) = info.host_id else {
        return Ok(None);
    };
    let resolved = state.store.resolve_host(host_id)?;
    Ok(resolved
        .identity
        .filter(|i| i.data.password.as_deref().is_some_and(|p| !p.is_empty()))
        .map(|i| {
            if i.data.label.trim().is_empty() {
                i.data.username.clone()
            } else {
                i.data.label.clone()
            }
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ls_command_quotes_and_resolves() {
        assert_eq!(
            remote_ls_command(Some("/home/me"), "src/it's"),
            "cd '/home/me' 2>/dev/null && LC_ALL=C ls -1apL -- 'src/it'\\''s' 2>/dev/null | head -n 2002"
        );
        assert_eq!(
            remote_ls_command(Some("/x"), "/etc/"),
            "LC_ALL=C ls -1apL -- '/etc/' 2>/dev/null | head -n 2002"
        );
        assert_eq!(
            remote_ls_command(None, "~/.ssh"),
            "cd \"$HOME\" 2>/dev/null && LC_ALL=C ls -1apL -- './.ssh' 2>/dev/null | head -n 2002"
        );
        assert!(remote_ls_command(None, "~").contains("-- './'"));
    }

    #[test]
    fn ls_output_parsed() {
        let entries = parse_ls("./\n../\n.git/\nCargo.toml\nsrc/\n");
        assert_eq!(entries.len(), 3);
        assert!(entries[0].dir && entries[0].name == ".git");
        assert!(!entries[1].dir && entries[1].name == "Cargo.toml");
    }

    #[test]
    fn local_listing() {
        let dir = std::env::temp_dir().join(format!("termoso-complete-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("file.txt"), b"x").unwrap();
        let entries = list_local(Some(dir.to_str().unwrap()), ".").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.name == "sub" && e.dir));
        assert!(entries.iter().any(|e| e.name == "file.txt" && !e.dir));
        assert!(list_local(None, "/definitely/not/here").unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
