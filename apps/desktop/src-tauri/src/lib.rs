//! Termoso desktop: Tauri 2 shell. Rust owns state, storage, transport and
//! sessions; the webview only renders and forwards user input.

mod account;
mod commands;
mod commands_tools;
mod error;
mod forwarding;
mod hosts;
mod keychain;
mod logs;
mod prompts;
mod sessions;
mod sftp;
mod snippets;
mod state;
mod trust;
mod update;

use tauri::Manager;

use crate::state::AppState;
use crate::update::UpdateHub;

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let state = AppState::open().map_err(|e| {
                tracing::error!(error = %e, "cannot open profile");
                std::io::Error::other(e.to_string())
            })?;
            tracing::info!(
                profile = %state.profile_dir.display(),
                master = ?state.master_source(),
                "profile opened"
            );
            app.manage(state);
            app.manage(UpdateHub::default());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(startup(handle));
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
            commands::hosts_delete,
            commands::host_duplicate,
            commands::hosts_move,
            commands::hosts_copy_to_vault,
            commands::host_inherited,
            commands::groups_list,
            commands::group_save,
            commands::group_form,
            commands::group_save_form,
            commands::group_duplicate,
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
            commands::sftp_sessions_list,
            commands::sftp_open,
            commands::sftp_close,
            commands::sftp_list,
            commands::sftp_stat,
            commands::sftp_mkdir,
            commands::sftp_rename,
            commands::sftp_remove,
            commands::sftp_chmod,
            commands::local_home,
            commands::local_list,
            commands::local_stat,
            commands::local_mkdir,
            commands::local_rename,
            commands::local_remove,
            commands::transfer_start,
            commands::transfer_cancel,
            commands_tools::keys_list,
            commands_tools::key_generate,
            commands_tools::key_import,
            commands_tools::key_import_file,
            commands_tools::key_rename,
            commands_tools::key_change_passphrase,
            commands_tools::key_remember_passphrase,
            commands_tools::key_public,
            commands_tools::key_export,
            commands_tools::key_export_file,
            commands_tools::key_delete,
            commands_tools::identities_list,
            commands_tools::identity_save,
            commands_tools::identity_delete,
            commands_tools::master_key_migrate,
            commands_tools::pf_rules,
            commands_tools::pf_save,
            commands_tools::pf_runtimes,
            commands_tools::pf_start,
            commands_tools::pf_stop,
            commands_tools::pf_delete,
            commands_tools::snippets_list,
            commands_tools::snippet_save,
            commands_tools::snippet_delete,
            commands_tools::snippet_run,
            commands_tools::snippet_packages,
            commands_tools::snippet_package_save,
            commands_tools::snippet_package_delete,
            commands_tools::known_hosts_list,
            commands_tools::known_host_forget,
            commands_tools::known_host_forget_host,
            commands_tools::known_hosts_import_text,
            commands_tools::known_hosts_import_file,
            commands_tools::known_hosts_export_text,
            commands_tools::known_hosts_export_file,
            commands_tools::known_hosts_default_path,
            commands_tools::logs_list,
            commands_tools::log_read,
            commands_tools::log_export,
            commands_tools::log_delete,
            commands_tools::log_bookmarks,
            commands_tools::log_bookmark_add,
            commands_tools::log_bookmark_delete,
            commands_tools::account_status,
            commands_tools::account_server_info,
            commands_tools::account_login,
            commands_tools::account_mfa,
            commands_tools::account_mfa_email_send,
            commands_tools::account_webauthn_challenge,
            commands_tools::account_device_approve,
            commands_tools::account_device_resend,
            commands_tools::account_cancel_login,
            commands_tools::account_register,
            commands_tools::account_sign_out,
            commands_tools::account_sync_now,
            commands_tools::account_devices,
            commands_tools::account_device_revoke,
            commands_tools::update_check,
            commands_tools::update_install,
            commands_tools::update_restart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Termoso");
}

/// Background work after the window is up: restore the account session,
/// sweep old recordings and bring up auto-start forwards.
async fn startup(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings().unwrap_or_default();
    match logs::prune(&state.store, settings.log_retention_days) {
        Ok(n) if n > 0 => tracing::info!(pruned = n, "old recordings removed"),
        Ok(_) => {}
        Err(e) => tracing::warn!("log retention sweep failed: {e}"),
    }
    match account::resume(&app).await {
        Ok(Some(a)) => tracing::info!(email = %a.email, "account session restored"),
        Ok(None) => {}
        Err(e) => tracing::warn!("account resume failed: {e}"),
    }
    if settings.autostart_forwarding {
        match forwarding::autostart(&app).await {
            Ok(ids) if !ids.is_empty() => {
                tracing::info!(count = ids.len(), "forwarding rules auto-started")
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("forwarding autostart failed: {e}"),
        }
    }
    if settings.update_check == "startup" {
        update::check_on_startup(&app).await;
    }
}
