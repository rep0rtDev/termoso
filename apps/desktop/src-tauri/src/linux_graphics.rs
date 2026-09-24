//! WebKitGTK's DMA-BUF renderer asks the GPU driver for buffer formats the
//! proprietary NVIDIA driver does not provide; the webview then paints
//! nothing (a blank / see-through window), flickers or dies on resize
//! (tauri-apps/tauri#9394, WebKit bug 261874). Disabling that renderer
//! keeps the app usable at the cost of the faster presentation path, so it
//! is done only when the NVIDIA kernel module is loaded and the user has not
//! decided for themselves via `WEBKIT_DISABLE_DMABUF_RENDERER` (`=0` keeps
//! the renderer on).

use std::path::Path;

const DMABUF_VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

fn nvidia_loaded() -> bool {
    Path::new("/proc/driver/nvidia/version").exists() || Path::new("/sys/module/nvidia").exists()
}

/// Must run before GTK / WebKit initialise and before any other thread starts.
pub fn apply_workarounds() {
    if std::env::var_os(DMABUF_VAR).is_some() || !nvidia_loaded() {
        return;
    }
    // SAFETY: called from `run()` on the main thread before any thread is
    // spawned, so nothing can read the environment concurrently.
    unsafe { std::env::set_var(DMABUF_VAR, "1") };
    tracing::info!("NVIDIA driver detected: WebKitGTK DMA-BUF renderer disabled");
}
