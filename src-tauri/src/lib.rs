use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const INJECT_SCRIPT: &str = r#"
(function() {
  window.addEventListener('keydown', function(e) {
    var mod = e.ctrlKey || e.metaKey;
    var key = e.key ? e.key.toLowerCase() : '';

    if (mod && key === 'l') {
      e.preventDefault();
      e.stopPropagation();
      window.location.href = window.__ZEB_DEV__ ? 'http://localhost:1420' : 'tauri://localhost';
      return;
    }

    if (e.key === 'Escape') {
      var tag = document.activeElement ? document.activeElement.tagName.toLowerCase() : '';
      if (tag !== 'input' && tag !== 'textarea') {
        e.preventDefault();
        e.stopPropagation();
        window.location.href = window.__ZEB_DEV__ ? 'http://localhost:1420' : 'tauri://localhost';
        return;
      }
    }

    if ((mod && key === 'r') || e.key === 'F5') {
      e.preventDefault();
      e.stopPropagation();
      window.location.reload();
      return;
    }

    if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      window.history.back();
      return;
    }

    if (e.altKey && e.key === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      window.history.forward();
      return;
    }
  }, true);

  function injectHomeBtn() {
    if (window.location.hostname === 'localhost' || window.location.origin.indexOf('tauri://') === 0) return;
    if (document.getElementById('zeb-floating-home')) return;

    var btn = document.createElement('button');
    btn.id = 'zeb-floating-home';
    btn.innerHTML = '⌘';
    btn.title = 'Home (Ctrl+L / Esc)';
    btn.style.cssText = 'position:fixed;bottom:16px;right:16px;z-index:2147483647;width:38px;height:38px;border-radius:10px;background:#ffffff;color:#1e293b;border:1px solid #cbd5e1;box-shadow:0 4px 16px rgba(0,0,0,0.15);font-size:16px;font-weight:bold;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:transform 0.15s;outline:none;user-select:none;font-family:sans-serif;';
    btn.onmouseenter = function() { btn.style.transform = 'scale(1.08)'; };
    btn.onmouseleave = function() { btn.style.transform = 'scale(1)'; };
    btn.onclick = function(e) {
      e.preventDefault();
      e.stopPropagation();
      window.location.href = window.__ZEB_DEV__ ? 'http://localhost:1420' : 'tauri://localhost';
    };

    if (document.body) {
      document.body.appendChild(btn);
    } else {
      document.addEventListener('DOMContentLoaded', function() {
        if (document.body && !document.getElementById('zeb-floating-home')) {
          document.body.appendChild(btn);
        }
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', injectHomeBtn);
  } else {
    injectHomeBtn();
  }
})();
"#;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn navigate_browser(app: AppHandle, url: String) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
    let target_url: tauri::Url = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;
    window.navigate(target_url).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn go_home(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
    #[cfg(dev)]
    let home_url: tauri::Url = "http://localhost:1420".parse().unwrap();
    #[cfg(not(dev))]
    let home_url: tauri::Url = "tauri://localhost".parse().unwrap();
    window.navigate(home_url).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn browser_reload(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
    window.reload().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn browser_go_back(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
    window.eval("window.history.back()").map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn browser_go_forward(app: AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
    window.eval("window.history.forward()").map_err(|e| e.to_string())?;
    Ok(())
}
/// WebKitGTK's DMA-BUF renderer fails on some Linux Wayland/Mesa/NVIDIA setups
/// with: `Could not create default EGL display: EGL_BAD_PARAMETER` and a blank window.
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
        .invoke_handler(tauri::generate_handler![
            greet,
            navigate_browser,
            go_home,
            browser_reload,
            browser_go_back,
            browser_go_forward
        ])
        .setup(|app| {
            let is_dev = cfg!(debug_assertions);
            let home_url = if is_dev {
                WebviewUrl::External("http://localhost:1420".parse().unwrap())
            } else {
                WebviewUrl::App("index.html".into())
            };

            let init_script = format!(
                "window.__ZEB_DEV__ = {};\n{}",
                if is_dev { "true" } else { "false" },
                INJECT_SCRIPT
            );

            let _window = WebviewWindowBuilder::new(app, "main", home_url)
                .title("zeb")
                .inner_size(1200.0, 800.0)
                .min_inner_size(800.0, 500.0)
                .resizable(true)
                .decorations(false)
                .transparent(false)
                .shadow(true)
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .devtools(true)
                .initialization_script(&init_script)
                .build()?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
