use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Lightweight JS-level tracker / ad blocker injected at document-start on every
/// non-local page. The dominant reason a heavy Vercel portfolio (React + many
/// blog components + Vercel Analytics + Sentry + etc.) pegs CPU inside zeb is
/// not the React tree itself — it is the third-party analytics / ad scripts
/// running in parallel. Short-circuiting them at fetch / XHR / script-tag time
/// is the single biggest runtime win we can give the webview without touching
/// the portfolio source.
///
/// Scope: only kicks in on non-local pages (skips localhost and tauri://).
/// Opt-out: set `ZEB_DISABLE_CONTENT_BLOCKER=1` before launching zeb.
const CONTENT_BLOCKER: &str = r#"
(function() {
  if (window.location.hostname === 'localhost' || window.location.origin.indexOf('tauri://') === 0) return;
  if (window.__ZEB_NO_BLOCK__ === true) return;

  // Curated list of the most common analytics / tracking / ad domains.
  // Keep alphabetical-ish per group for easy review.
  var BLOCKED = [
    // Google analytics / ads
    'google-analytics.com','googletagmanager.com','analytics.google.com',
    'doubleclick.net','googlesyndication.com','googleadservices.com',
    'adservice.google.com','googletagservices.com',
    // Facebook / Meta
    'connect.facebook.net','facebook.com','facebook.net',
    // Hotjar / FullStory / LogRocket / Mouseflow / ClickTale (session replay)
    'static.hotjar.com','script.hotjar.com','vc.hotjar.io',
    'fullstory.com','edr.fullstory.com',
    'logrocket.com','cdn.logrocket.io','r.logrocket.io',
    'mouseflow.com','sessioncam.com','clicktale.net',
    // Product analytics
    'cdn.mxpnl.com','api.mixpanel.com','mixpanel.com',
    'cdn.segment.com','api.segment.io','segment.com',
    'api.amplitude.com','amplitude.com',
    'heapanalytics.com','heap.io',
    // Error monitoring (often no-op in dev but still costs main-thread)
    'browser.sentry-cdn.com','o*.sentry.io','ingest.sentry.io',
    'js-agent.newrelic.com','bam.nr-data.net',
    'datadoghq.com','browser-intake-datadoghq.com',
    // Customer chat widgets
    'widget.intercom.io','js.intercomcdn.com','intercom.io',
    'widget.zdassets.com','static.zdassets.com',
    'embed.tawk.to','va.tawk.to',
    // Privacy-friendly but still third-party
    'plausible.io','umami.is','analytics.umami.is',
    // Vercel / Next.js built-in
    'va.vercel-scripts.com','vitals.vercel-insights.com',
    // Cloudflare
    'cloudflareinsights.com','static.cloudflareinsights.com',
    // Yandex / Quantcast / Comscore
    'mc.yandex.ru','mc.yandex.com',
    'quantserve.com','scorecardresearch.com',
    // Ad exchanges
    'adnxs.com','rubiconproject.com','casalemedia.com',
    'pubmatic.com','openx.net','criteo.com','criteo.net',
    'bidr.io','moatads.com','taboola.com','outbrain.com',
  ];

  function isBlocked(u) {
    try {
      var h = new URL(u, location.href).hostname.toLowerCase();
      for (var i = 0; i < BLOCKED.length; i++) {
        var d = BLOCKED[i];
        if (h === d || h.substr(h.length - d.length - 1) === '.' + d) return true;
      }
    } catch (e) {}
    return false;
  }

  // 1. fetch()
  var _f = window.fetch;
  if (typeof _f === 'function') {
    window.fetch = function(u, o) {
      if (typeof u === 'string' && isBlocked(u)) {
        return Promise.resolve(new Response('', {status: 200, statusText: 'Blocked by zeb'}));
      }
      return _f.apply(this, arguments);
    };
  }

  // 2. XMLHttpRequest
  var _X = window.XMLHttpRequest;
  if (typeof _X === 'function') {
    function NX() {
      var x = new _X();
      var _o = x.open;
      var _s = x.send;
      x._zebBlocked = false;
      x.open = function(m, u) {
        if (typeof u === 'string' && isBlocked(u)) { x._zebBlocked = true; return; }
        return _o.apply(this, arguments);
      };
      x.send = function() { if (x._zebBlocked) return; return _s.apply(this, arguments); };
      return x;
    }
    NX.prototype = _X.prototype;
    Object.defineProperty(NX, 'name', { value: 'XMLHttpRequest' });
    window.XMLHttpRequest = NX;
  }

  // 3. sendBeacon (best-effort)
  if (typeof navigator !== 'undefined' && typeof navigator.sendBeacon === 'function') {
    var _b = navigator.sendBeacon.bind(navigator);
    navigator.sendBeacon = function(u, d) {
      if (typeof u === 'string' && isBlocked(u)) return true;
      return _b(u, d);
    };
  }

  // 4. dynamically inserted <script src="..."> tags
  if (typeof MutationObserver !== 'undefined' && document.documentElement) {
    new MutationObserver(function(rs) {
      rs.forEach(function(r) {
        r.addedNodes.forEach(function(n) {
          if (n && n.tagName === 'SCRIPT' && n.src && isBlocked(n.src)) {
            n.remove();
          }
        });
      });
    }).observe(document.documentElement, {childList: true, subtree: true});
  }
})();
"#;

/// Lite Mode — applied to every non-local page when `ZEB_LITE=1`. This is the
/// "make it feel smooth" knob. The four things it does, in order of impact:
///
/// 1. Forces `prefers-reduced-motion: reduce` via a `!important` stylesheet,
///    killing CSS animations / transitions / smooth-scroll. Single biggest
///    jank killer on a React + Framer-Motion / GSAP portfolio.
/// 2. Sets `loading="lazy"` and `decoding="async"` on every <img> on the page
///    and on every <img> that gets added later via a MutationObserver. Stops
///    the browser from decoding 50 hero images on first paint.
/// 3. Disables `WebGL` / `WebGL2` contexts. Most portfolios don't need them
///    and the GPU context init alone is a multi-hundred-ms cost on software
///    render.
/// 4. Adds a `zeb-lite` class on <html> so the user (or site CSS) can target
///    it from DevTools / their own stylesheet.
const LITE_MODE: &str = r#"
(function() {
  if (window.__ZEB_LITE__ !== true) return;
  if (window.location.hostname === 'localhost' || window.location.origin.indexOf('tauri://') === 0) return;

  // 1. force reduced motion
  var s = document.createElement('style');
  s.id = 'zeb-lite-style';
  s.textContent = '*,*::before,*::after{animation-duration:0.001ms!important;animation-delay:0ms!important;animation-iteration-count:1!important;transition-duration:0.001ms!important;transition-delay:0ms!important;scroll-behavior:auto!important}';
  (document.head || document.documentElement).appendChild(s);

  // 2. lazy-load every <img>, including ones added later
  function lazy() {
    var imgs = document.querySelectorAll('img');
    for (var i = 0; i < imgs.length; i++) {
      if (!imgs[i].loading) imgs[i].loading = 'lazy';
      if (!imgs[i].decoding) imgs[i].decoding = 'async';
    }
  }
  lazy();
  if (typeof MutationObserver !== 'undefined' && document.documentElement) {
    new MutationObserver(lazy).observe(document.documentElement, {childList: true, subtree: true});
  }

  // 3. disable WebGL contexts (3D libs / shaders are a huge cost on software render)
  try {
    var origGetContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function(type) {
      if (type === 'webgl' || type === 'webgl2' || type === 'experimental-webgl') {
        return null;
      }
      return origGetContext.apply(this, arguments);
    };
  } catch (e) {}

  // 4. body marker for DevTools / user CSS
  try { document.documentElement.classList.add('zeb-lite'); } catch (e) {}
})();
"#;

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

/// Surfaces the current performance / blocker env-var state to the React UI
/// so the small "?" settings chip in the spotlight can render what's on.
#[tauri::command]
fn get_perf_settings() -> serde_json::Value {
    serde_json::json!({
        "hardware_accel": env_flag_true("ZEB_HARDWARE_ACCEL"),
        "content_blocker_disabled": env_flag_true("ZEB_DISABLE_CONTENT_BLOCKER"),
        "lite_mode": env_flag_true("ZEB_LITE"),
    })
}

fn env_flag_true(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("on") | Some("yes")
    )
}

/// WebKitGTK environment knobs applied at process start.
///
/// `WEBKIT_DISABLE_DMABUF_RENDERER=1` was originally forced unconditionally
/// (see Problem 3 in `agent.md`) to dodge the
/// `Could not create default EGL display: EGL_BAD_PARAMETER` crash on certain
/// Wayland / Mesa / NVIDIA stacks. The side effect is that WebKit falls back
/// to a software render pipeline — which is the dominant reason a heavy site
/// (like a Vercel portfolio full of React + many blog components + analytics)
/// pegs the CPU and freezes the UI inside zeb.
///
/// We now expose an opt-in (`ZEB_HARDWARE_ACCEL=1`) so users on systems that
/// do NOT exhibit the EGL crash can re-enable GPU rendering. The default is
/// kept on the safe software path so the existing crash fix is preserved.
#[cfg(target_os = "linux")]
fn apply_webkit_env() {
    let want_hardware = env_flag_true("ZEB_HARDWARE_ACCEL");

    if want_hardware {
        // User explicitly opted in: leave WEBKIT_DISABLE_DMABUF_RENDERER alone
        // so WebKit can pick DMA-BUF / GPU when the system supports it.
        // If the user previously set it to 1, that's their choice — we don't
        // override an explicit value.
        eprintln!("[zeb] hardware acceleration: ENABLED (ZEB_HARDWARE_ACCEL=1)");
    } else {
        #[allow(unused_unsafe)]
        unsafe {
            if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
        eprintln!("[zeb] hardware acceleration: disabled (software render — set ZEB_HARDWARE_ACCEL=1 to opt in)");
    }

    // Encourage disk cache for repeat visits to heavy sites.
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        let _ = std::env::set_var("WEBKIT_CACHE_DIR", format!("{}/zeb", xdg));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_webkit_env();

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
            browser_go_forward,
            get_perf_settings
        ])
        .setup(|app| {
            let is_dev = cfg!(debug_assertions);
            let home_url = if is_dev {
                WebviewUrl::External("http://localhost:1420".parse().unwrap())
            } else {
                WebviewUrl::App("index.html".into())
            };

            // Opt-outs / opt-ins from the env.
            let no_block = env_flag_true("ZEB_DISABLE_CONTENT_BLOCKER");
            let lite = env_flag_true("ZEB_LITE");

            // Compose init script: env flags first, then content blocker,
            // then lite mode, then keyboard / floating-home-button script.
            let init_script = format!(
                "window.__ZEB_DEV__ = {};\nwindow.__ZEB_NO_BLOCK__ = {};\nwindow.__ZEB_LITE__ = {};\n{}\n{}\n{}",
                if is_dev { "true" } else { "false" },
                if no_block { "true" } else { "false" },
                if lite { "true" } else { "false" },
                CONTENT_BLOCKER,
                LITE_MODE,
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
