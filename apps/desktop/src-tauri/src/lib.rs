//! Termoso desktop: Tauri 2 shell. Rust owns state, storage, transport and
//! sessions; the webview only renders and forwards user input.

mod account;
mod commands;
mod commands_tools;
mod complete;
mod edits;
mod error;
mod forwarding;
mod hosts;
mod import;
mod keychain;
mod logs;
mod prompts;
mod sessions;
mod sftp;
mod snippets;
mod state;
mod trust;
mod update;
mod workspaces;

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

    let mut context = tauri::generate_context!();
    // WebKitGTK strips file:// URIs from HTML5 drops, so OS drops need the
    // native handler there; WebView2 needs it off for HTML5 drag-and-drop.
    for window in &mut context.config_mut().app.windows {
        window.drag_drop_enabled = cfg!(not(windows));
    }

    tauri::Builder::default()
        // First, so a second launch (e.g. the OS opening an ssh:// link) hands
        // its arguments to the running instance instead of starting another.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
            commands::deep_links_register,
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
            commands::serial_ports,
            commands::local_shells,
            commands::hosts_copy_to_vault,
            commands::host_inherited,
            commands::groups_list,
            commands::group_save,
            commands::group_form,
            commands::group_save_form,
            commands::group_duplicate,
            commands::group_delete,
            commands::tags_list,
            commands::tag_update,
            commands::tag_delete,
            commands::tags_merge,
            commands::history_connections,
            commands::history_commands,
            commands::history_record_command,
            commands::history_delete,
            commands::history_clear_commands,
            complete::terminal_list_dir,
            complete::terminal_insert_password,
            complete::terminal_host_identity,
            workspaces::workspaces_get,
            workspaces::workspaces_set,
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
            commands::local_drives,
            commands::local_stat,
            commands::local_mkdir,
            commands::local_rename,
            commands::local_remove,
            commands::local_open,
            commands::transfer_probe,
            commands::transfer_start,
            commands::transfer_cancel,
            commands::transfer_pause,
            commands::transfer_resume,
            commands::transfer_forget,
            commands::drop_begin,
            commands::drop_write,
            commands::drop_mkdir,
            commands::drop_abort,
            commands::edits_list,
            commands::edit_open,
            commands::edit_upload_now,
            commands::edit_close,
            commands_tools::keys_list,
            commands_tools::key_generate,
            commands_tools::key_import,
            commands_tools::key_import_file,
            commands_tools::key_inspect,
            commands_tools::key_inspect_file,
            commands_tools::certificate_inspect,
            commands_tools::certificate_inspect_file,
            commands_tools::key_certificate,
            commands_tools::key_set_certificate,
            commands_tools::key_set_certificate_file,
            commands_tools::key_copy_to_vault,
            commands_tools::key_rename,
            commands_tools::key_change_passphrase,
            commands_tools::key_remember_passphrase,
            commands_tools::key_public,
            commands_tools::key_export,
            commands_tools::key_export_file,
            commands_tools::key_export_to_host,
            commands_tools::agent_keys,
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
            commands_tools::pf_duplicate,
            commands_tools::pf_copy_to_vault,
            commands_tools::snippets_list,
            commands_tools::snippet_save,
            commands_tools::snippet_delete,
            commands_tools::snippet_set_targets,
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
            commands_tools::import_scan_ssh,
            commands_tools::import_parse_file,
            commands_tools::import_scan_putty_registry,
            commands_tools::import_ssh_dir_default,
            commands_tools::import_csv_template,
            commands_tools::import_csv_template_save,
            commands_tools::import_apply,
            commands_tools::import_discard,
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
            commands_tools::account_vault_members,
            commands_tools::update_check,
            commands_tools::update_install,
            commands_tools::update_restart,
        ])
        .build(context)
        .expect("error while building Termoso")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().edits.close_all();
            }
        });
}

/// Background work after the window is up: restore the account session,
/// sweep old recordings and bring up auto-start forwards.
async fn startup(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings().unwrap_or_default();
    edits::sweep_stale();
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
