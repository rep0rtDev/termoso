//! Termoso desktop: Tauri 2 shell. Rust owns state, storage, transport and
//! sessions; the webview only renders and forwards user input.

mod commands;
mod error;
mod hosts;
mod prompts;
mod sessions;
mod state;

use tauri::Manager;

use crate::state::AppState;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("TERMOSO_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::open().map_err(|e| {
                tracing::error!(error = %e, "cannot open profile");
                std::io::Error::other(e.to_string())
            })?;
            tracing::info!(
                profile = %state.profile_dir.display(),
                master = ?state.master_source,
                "profile opened"
            );
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::vaults_list,
            commands::vault_default,
            commands::entities_list,
            commands::entity_get,
            commands::entity_save,
            commands::entity_delete,
            commands::entity_move,
            commands::hosts_list,
            commands::host_form,
            commands::host_save,
            commands::host_delete,
            commands::groups_list,
            commands::group_save,
            commands::group_delete,
            commands::tags_list,
            commands::history_connections,
            commands::sessions_list,
            commands::terminal_open,
            commands::terminal_attach,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
            commands::prompt_answer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Termoso");
}
