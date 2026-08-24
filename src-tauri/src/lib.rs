use tauri::{AppHandle, Emitter, Manager, WebviewUrl, webview::WebviewBuilder, LogicalPosition, LogicalSize};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn navigate_browser(app: AppHandle, url: String) -> Result<(), String> {
    let window = app.get_window("main").ok_or("Main window not found")?;
    let target_url: tauri::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;

    if let Some(browser_view) = app.get_webview("browser") {
        browser_view.navigate(target_url).map_err(|e| e.to_string())?;
        browser_view.show().map_err(|e| e.to_string())?;
    } else {
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let webview_builder = WebviewBuilder::new("browser", WebviewUrl::External(target_url))
            .auto_resize();
        window.add_child(
            webview_builder,
            LogicalPosition::new(0, 0),
            LogicalSize::new(size.width, size.height),
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn close_browser(app: AppHandle) -> Result<(), String> {
    if let Some(browser_view) = app.get_webview("browser") {
        browser_view.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn browser_reload(app: AppHandle) -> Result<(), String> {
    if let Some(browser_view) = app.get_webview("browser") {
        browser_view.reload().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn browser_go_back(app: AppHandle) -> Result<(), String> {
    if let Some(browser_view) = app.get_webview("browser") {
        browser_view.eval("window.history.back()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn browser_go_forward(app: AppHandle) -> Result<(), String> {
    if let Some(browser_view) = app.get_webview("browser") {
        browser_view.eval("window.history.forward()").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn set_browser_bounds(app: AppHandle, top: f64) -> Result<(), String> {
    let window = app.get_window("main").ok_or("Main window not found")?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    if let Some(browser_view) = app.get_webview("browser") {
        let height = (size.height as f64 - top).max(50.0);
        let _ = browser_view.set_position(LogicalPosition::new(0.0, top));
        let _ = browser_view.set_size(LogicalSize::new(size.width as f64, height));
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            navigate_browser,
            close_browser,
            browser_reload,
            browser_go_back,
            browser_go_forward,
            set_browser_bounds
        ])
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
                    if let Some(browser) = app.get_webview("browser") {
                        let _ = browser.reload();
                    }
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("reload-page", ());
                    }
                });
                // Ctrl+R -> reload
                let ctrl_r = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyR);
                let _ = app.global_shortcut().on_shortcut(ctrl_r, |app, _, _| {
                    if let Some(browser) = app.get_webview("browser") {
                        let _ = browser.reload();
                    }
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("reload-page", ());
                    }
                });
                // Alt+Left -> back
                let back = Shortcut::new(Some(Modifiers::ALT), Code::ArrowLeft);
                let _ = app.global_shortcut().on_shortcut(back, |app, _, _| {
                    if let Some(browser) = app.get_webview("browser") {
                        let _ = browser.eval("window.history.back()");
                    }
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.emit("go-back", ());
                    }
                });
                // Alt+Right -> forward
                let fwd = Shortcut::new(Some(Modifiers::ALT), Code::ArrowRight);
                let _ = app.global_shortcut().on_shortcut(fwd, |app, _, _| {
                    if let Some(browser) = app.get_webview("browser") {
                        let _ = browser.eval("window.history.forward()");
                    }
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
