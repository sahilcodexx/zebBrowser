// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::{Emitter, Manager};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// WebKitGTK's DMA-BUF renderer fails inside AppImages on Wayland compositors
/// (Hyprland, NVIDIA, some Intel Mesa) with:
/// `Could not create default EGL display: EGL_BAD_PARAMETER` and a blank window.
/// Must run before gtk/webkit init.
#[cfg(target_os = "linux")]
fn disable_webkit_dmabuf() {
    #[allow(unused_unsafe)]
    unsafe {
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    disable_webkit_dmabuf();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                // Ctrl+L -> focus url bar (also Cmd+L on mac)
                let ctrl_l = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyL);
                let _ = app.global_shortcut().on_shortcut(ctrl_l, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("focus-url", ());
                    }
                });
                // F5 -> reload
                let f5 = Shortcut::new(None, Code::F5);
                let _ = app.global_shortcut().on_shortcut(f5, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("reload-page", ());
                    }
                });
                // Ctrl+R -> reload
                let ctrl_r = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyR);
                let _ = app.global_shortcut().on_shortcut(ctrl_r, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("reload-page", ());
                    }
                });
                // Alt+Left -> back
                let back = Shortcut::new(Some(Modifiers::ALT), Code::ArrowLeft);
                let _ = app.global_shortcut().on_shortcut(back, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("go-back", ());
                    }
                });
                // Alt+Right -> forward
                let fwd = Shortcut::new(Some(Modifiers::ALT), Code::ArrowRight);
                let _ = app.global_shortcut().on_shortcut(fwd, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("go-forward", ());
                    }
                });
                // Escape -> hide
                let esc = Shortcut::new(None, Code::Escape);
                let _ = app.global_shortcut().on_shortcut(esc, |app, _, _| {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("hide-bars", ());
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
