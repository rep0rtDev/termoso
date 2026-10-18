//! Quick actions outside the window: files handed to the app by the OS, the
//! macOS Dock menu and the Windows Jump List. Linux desktop actions live in
//! `templates/termoso.desktop`; all of them end up as `termoso://new|local|sftp`
//! links or a `files-opened` event the web view routes.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Runtime};

pub const FILES_EVENT: &str = "files-opened";

/// Paths from the first launch's argv, handed to the web view when it asks.
#[derive(Default)]
pub struct LaunchFiles(Mutex<Vec<String>>);

impl LaunchFiles {
    pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Self {
        Self(Mutex::new(file_args(args)))
    }

    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Arguments that are existing files (URLs and flags go to the deep-link plugin).
pub fn file_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter()
        .filter(|a| !a.starts_with('-') && !a.contains("://"))
        .filter(|a| PathBuf::from(a).is_file())
        .collect()
}

/// A second instance started with files: forward them to the running window.
pub fn forward_files<R: Runtime>(app: &AppHandle<R>, args: &[String]) {
    let files = file_args(args.iter().skip(1).cloned());
    if !files.is_empty() {
        let _ = app.emit(FILES_EVENT, files);
    }
}

/// macOS "open with" / drop on the Dock icon: `file://` URLs.
#[cfg(target_os = "macos")]
pub fn forward_urls<R: Runtime>(app: &AppHandle<R>, urls: &[tauri::Url]) {
    let files: Vec<String> = urls
        .iter()
        .filter_map(|u| u.to_file_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !files.is_empty() {
        let _ = app.emit(FILES_EVENT, files);
    }
}

/// Dock menu with the three quick actions; picks are emitted as deep links.
#[cfg(target_os = "macos")]
pub fn install_dock_menu<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "quick:new", "New Connection", true, None::<&str>)?,
            &MenuItem::with_id(app, "quick:local", "Local Terminal", true, None::<&str>)?,
            &MenuItem::with_id(app, "quick:sftp", "SFTP", true, None::<&str>)?,
        ],
    )?;
    app.set_dock_menu(&menu)?;
    app.on_menu_event(|app, event| {
        if let Some(action) = event.id().as_ref().strip_prefix("quick:") {
            let _ = app.emit("quick-action", format!("termoso://{action}"));
        }
    });
    Ok(())
}

/// Taskbar Jump List tasks; each relaunches the exe with a `termoso://` link,
/// which the single-instance / deep-link plugins hand to the running window.
#[cfg(windows)]
pub fn install_jump_list() {
    use windows::Win32::System::Com::StructuredStorage::{
        InitPropVariantFromStringVector, PropVariantClear,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
    use windows::Win32::UI::Shell::{
        DestinationList, EnumerableObjectCollection, ICustomDestinationList, IObjectArray,
        IObjectCollection, IShellLinkW, ShellLink,
    };
    use windows::core::GUID;
    use windows::core::{HSTRING, Interface, PCWSTR};

    // PKEY_Title = {F29F85E0-4FF9-1068-AB91-08002B27B3D9}, 2
    const PKEY_TITLE: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9),
        pid: 2,
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let exe = HSTRING::from(exe.as_os_str());
    let tasks: [(&str, &str); 3] = [
        ("New connection", "termoso://new"),
        ("Local terminal", "termoso://local"),
        ("SFTP", "termoso://sftp"),
    ];
    let result: windows::core::Result<()> = (|| unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let list: ICustomDestinationList =
            CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;
        let collection: IObjectCollection =
            CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for (title, link) in tasks {
            let shell_link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            shell_link.SetPath(PCWSTR(exe.as_ptr()))?;
            shell_link.SetArguments(PCWSTR(HSTRING::from(link).as_ptr()))?;
            shell_link.SetIconLocation(PCWSTR(exe.as_ptr()), 0)?;
            let store: IPropertyStore = shell_link.cast()?;
            let mut value =
                InitPropVariantFromStringVector(Some(&[PCWSTR(HSTRING::from(title).as_ptr())]))?;
            store.SetValue(&PKEY_TITLE, &value)?;
            store.Commit()?;
            let _ = PropVariantClear(&mut value);
            collection.AddObject(&shell_link)?;
        }
        let array: IObjectArray = collection.cast()?;
        list.AddUserTasks(&array)?;
        list.CommitList()
    })();
    if let Err(e) = result {
        tracing::debug!(error = %e, "jump list not installed");
    }
}
