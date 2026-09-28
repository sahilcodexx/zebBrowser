//! Native adblocking via WebKitGTK's content-blocker (UserContentFilterStore)
//! plus cosmetic (element-hiding) filtering via user stylesheets.
//!
//! On first run EasyList is downloaded and split into:
//!   • network rules  → WebKit content-blocker JSON → compiled native filter
//!   • element-hiding → CSS injected as user stylesheets (global + per-domain)
//! Fully native — no proxy, no JS injection per request.

use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::path::PathBuf;

use crate::webkit_ffi as ffi;

const EASYLIST_URL: &str = "https://easylist.to/easylist/easylist.txt";
const FILTER_ID: &str = "easylist";
/// Max per-domain element-hiding domains kept (top by selector count).
const MAX_COSMETIC_DOMAINS: usize = 400;
/// Max global (unqualified) element-hiding selectors.
const MAX_GLOBAL_SELECTORS: usize = 3000;

fn data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local/share/light-browser")
}

fn network_json_path() -> PathBuf {
    // v4: valid resource-type names + css-display-none rules.
    data_dir().join("easylist.v4.json")
}

fn cosmetic_global_path() -> PathBuf {
    data_dir().join("cosmetic.global.v4.css")
}

fn cosmetic_domains_css_path() -> PathBuf {
    data_dir().join("cosmetic.domains.v4.css")
}

fn cosmetic_domains_list_path() -> PathBuf {
    data_dir().join("cosmetic.domains.v4.list")
}

fn filter_store_dir() -> PathBuf {
    data_dir().join("filters")
}

// ── EasyList → WebKit conversion ─────────────────────────────────────────────

pub struct AdblockData {
    pub network_json: String,
    pub global_css: String,
    /// (domain glob like "*://*.example.com/*", ) parallel to domain_css
    pub cosmetic_domains: Vec<String>,
    pub domain_css: String,
}

/// Convert EasyList text into WebKit content-blocker JSON + cosmetic CSS.
fn easylist_to_filters(text: &str) -> AdblockData {
    let mut rules: Vec<String> = Vec::new();
    let mut global_selectors: Vec<String> = Vec::new();
    let mut domain_selectors: HashMap<String, Vec<String>> = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with('[') || line == "#" {
            continue; // comments / metadata
        }

        // ── Cosmetic (element-hiding) rules ──
        if let Some((lhs, rhs)) = split_cosmetic(line) {
            // `#@#` is an *exception* hiding rule (un-hide); skipping keeps
            // generic hiding active, which is the safe default.
            if is_cosmetic_exception(line) {
                continue;
            }
            let selector = rhs.trim().to_string();
            if selector.is_empty() {
                continue;
            }
            let domains = lhs.trim_end();
            if domains.is_empty() {
                // Global hiding rule.
                if global_selectors.len() < MAX_GLOBAL_SELECTORS {
                    global_selectors.push(selector);
                }
            } else if !domains.contains('~') && domains.len() < 128 {
                for d in domains.split(',') {
                    let d = d.trim();
                    if !d.is_empty() {
                        domain_selectors
                            .entry(d.to_string())
                            .or_default()
                            .push(selector.clone());
                    }
                }
            }
            continue;
        }

        // ── Network rules ──
        if line.starts_with('#') {
            continue;
        }

        let mut is_exception = false;
        let mut rule = line.to_string();
        if let Some(rest) = rule.strip_prefix("@@") {
            is_exception = true;
            rule = rest.to_string();
        }

        // Options after '$': keep only the ones we can express.
        let mut third_party = false;
        let mut resource_types: Vec<&str> = Vec::new();
        if let Some(dollar) = rule.find('$') {
            let opts: Vec<&str> = rule[dollar + 1..].split(',').collect();
            let mut supported = true;
            for o in &opts {
                match *o {
                    "third-party" => third_party = true,
                    "script" => resource_types.push("script"),
                    "image" => resource_types.push("image"),
                    "stylesheet" => resource_types.push("style-sheet"),
                    "media" => resource_types.push("media"),
                    "font" => resource_types.push("font"),
                    "xmlhttprequest" => resource_types.push("raw"),
                    "subdocument" => resource_types.push("document"),
                    "popup" => resource_types.push("popup"),
                    "document" | "~third-party" | "~script" | "~image"
                    | "~stylesheet" | "~media" | "~font" | "~xmlhttprequest"
                    | "~subdocument" => {}
                    _ => {
                        supported = false;
                        break;
                    }
                }
            }
            if !supported {
                continue;
            }
            rule.truncate(dollar);
        }
        if rule.is_empty() {
            continue;
        }

        // Trailing `|` = ends-with anchor.
        let mut end_anchor = false;
        if rule.ends_with('|') {
            end_anchor = true;
            rule.pop();
        }
        if rule.is_empty() {
            continue;
        }

        let url_filter = if let Some(rest) = rule.strip_prefix("||") {
            escape_regex(rest)
        } else if let Some(rest) = rule.strip_prefix('|') {
            format!("^{}", escape_regex(rest))
        } else {
            escape_regex(&rule)
        };
        let url_filter = if end_anchor {
            format!("{url_filter}$")
        } else {
            url_filter
        };

        if url_filter.is_empty() {
            continue;
        }

        let action = if is_exception {
            "ignore-previous-rules"
        } else {
            "block"
        };
        let mut trigger = format!("\"url-filter\":\"{}\"", json_escape(&url_filter));
        // WebKit content-blocker format: trigger flags are ARRAYS.
        if third_party {
            trigger.push_str(",\"load-type\":[\"third-party\"]");
        }
        if !resource_types.is_empty() {
            let types = resource_types
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(",");
            trigger.push_str(&format!(",\"resource-type\":[{types}]"));
        }
        rules.push(format!(
            "{{\"action\":{{\"type\":\"{action}\"}},\"trigger\":{{{trigger}}}}}"
        ));
    }

    // Aggregate cosmetic CSS: top-N domains by selector count.
    let mut by_count: Vec<(String, Vec<String>)> = domain_selectors.into_iter().collect();
    by_count.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    by_count.truncate(MAX_COSMETIC_DOMAINS);

    // Domain-qualified hiding via native css-display-none actions
    // (if-domain trigger) — injected by the engine itself, no stylesheets.
    let mut css_domains: Vec<String> = Vec::new();
    let mut domain_css_parts: Vec<String> = Vec::new();
    for (domain, sels) in &by_count {
        let joined = sels.join(", ");
        rules.push(format!(
            "{{\"action\":{{\"type\":\"css-display-none\",\"selector\":\"{}\"}},\"trigger\":{{\"url-filter\":\".*\",\"if-domain\":[\"*{}\"]}}}}",
            json_escape(&joined),
            json_escape(domain)
        ));
        // Kept for the stylesheet fallback path (unused when rules succeed).
        css_domains.push(format!("*://*.{domain}/*"));
        domain_css_parts.push(css_hide(sels));
    }

    AdblockData {
        network_json: format!("[{}]", rules.join(",")),
        global_css: css_hide(&global_selectors),
        cosmetic_domains: css_domains,
        domain_css: domain_css_parts.join("\n"),
    }
}

/// Find the earliest element-hiding separator. Returns (lhs, rhs).
fn split_cosmetic(line: &str) -> Option<(&str, &str)> {
    for sep in ["#@#", "#?#", "#$#", "##"] {
        if let Some(pos) = line.find(sep) {
            // `#@#` is an exception (allow) rule — treat as cosmetic too but
            // callers skip because the check below handles ordering.
            return Some((&line[..pos], &line[pos + sep.len()..]));
        }
    }
    None
}

/// Exception hiding rules (`#@#`) must NOT hide anything; skip them.
fn is_cosmetic_exception(line: &str) -> bool {
    line.contains("#@#")
}

fn css_hide(selectors: &[String]) -> String {
    if selectors.is_empty() {
        return String::new();
    }
    let joined = selectors.join(",\n");
    format!("{joined} {{ display: none !important; visibility: hidden !important; }}\n")
}

/// WebKit's url-filter is a JS regex. EasyList → regex:
///   `^` separator wildcard, `*` global wildcard, everything else literal.
fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '.' => out.push_str("\\."),
            '+' => out.push_str("\\+"),
            '?' => out.push_str("\\?"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '|' => out.push_str("\\|"),
            '$' => out.push_str("\\$"),
            // Separator wildcard: any char that is not alnum/._%- (or end).
            '^' => out.push_str("([^a-zA-Z0-9._%+-]|$)"),
            // Global wildcard.
            '*' => out.push_str(".*"),
            other => out.push(other),
        }
    }
    out
}

/// Escape for embedding inside a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Download EasyList (blocking network I/O — runs on a background thread).
fn download_easylist() -> Result<String, String> {
    for (cmd, args) in [
        ("curl", vec!["-fsSL", "--max-time", "60", EASYLIST_URL]),
        ("wget", vec!["-qO-", "-T", "60", EASYLIST_URL]),
    ] {
        if let Ok(out) = std::process::Command::new(cmd).args(&args).output() {
            if out.status.success() && !out.stdout.is_empty() {
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
        }
    }
    Err("could not download EasyList (curl/wget missing or offline)".to_string())
}

/// Ensure all filter artifacts exist on disk. Returns paths on success.
fn ensure_filters() -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), String> {
    let net = network_json_path();
    let css_global = cosmetic_global_path();
    let css_domains = cosmetic_domains_css_path();
    let list_domains = cosmetic_domains_list_path();

    if net.exists() && css_global.exists() && css_domains.exists() && list_domains.exists() {
        return Ok((net, css_global, css_domains, list_domains));
    }

    if let Some(parent) = net.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let text = download_easylist()?;
    let data = easylist_to_filters(&text);

    std::fs::write(&net, data.network_json.as_bytes())
        .map_err(|e| format!("cannot write network filters: {e}"))?;
    std::fs::write(&css_global, data.global_css.as_bytes())
        .map_err(|e| format!("cannot write global css: {e}"))?;
    std::fs::write(&css_domains, data.domain_css.as_bytes())
        .map_err(|e| format!("cannot write domain css: {e}"))?;
    std::fs::write(&list_domains, data.cosmetic_domains.join("\n"))
        .map_err(|e| format!("cannot write domain list: {e}"))?;

    Ok((net, css_global, css_domains, list_domains))
}

// ── Shared state ─────────────────────────────────────────────────────────────

enum State {
    Idle,
    Loading(Vec<*mut c_void>), // pending user content managers
    Ready(FilterBundle),
    Failed,
}

/// Compiled network filter + cosmetic style sheets + scriptlets (app lifetime).
struct FilterBundle {
    filter: *mut c_void,
    style_sheets: Vec<*mut c_void>,
    user_scripts: Vec<*mut c_void>,
}

thread_local! {
    static SHARED: RefCell<State> = const { RefCell::new(State::Idle) };
}

/// Attach the adblock filters to a webview's user content manager. Call once
/// per webview right after creation. If the filter isn't ready yet, the
/// manager is queued and filters are attached when loading finishes.
pub fn attach_to_webview(wv_ptr: *mut ffi::WebKitWebView) {
    let manager = unsafe { ffi::webkit_web_view_get_user_content_manager(wv_ptr) };
    if manager.is_null() {
        return;
    }

    SHARED.with(|shared| {
        let mut state = shared.borrow_mut();
        match &mut *state {
            State::Ready(bundle) => unsafe {
                apply_bundle(bundle, manager);
            },
            State::Loading(pending) => pending.push(manager),
            State::Idle | State::Failed => {
                *state = State::Loading(vec![manager]);
                drop(state);
                start_download();
            }
        }
    });
}

unsafe fn apply_bundle(bundle: &FilterBundle, manager: *mut c_void) {
    if !bundle.filter.is_null() {
        ffi::webkit_user_content_manager_add_filter(manager, bundle.filter);
    }
    for sheet in &bundle.style_sheets {
        ffi::webkit_user_content_manager_add_style_sheet(manager, *sheet);
    }
    for script in &bundle.user_scripts {
        ffi::webkit_user_content_manager_add_script(manager, *script);
    }
}

fn start_download() {
    std::thread::spawn(|| {
        let paths = ensure_filters();
        gtk4::glib::MainContext::default().invoke(move || {
            load_into_store(paths);
        });
    });
}

/// GAsyncReadyCallback: the source object IS the filter store.
unsafe extern "C" fn load_trampoline(
    store: *mut ffi::GObject,
    result: *mut c_void,
    _data: *mut c_void,
) {
    finish_load(store as *mut c_void, result);
}

fn load_into_store(paths: Result<(PathBuf, PathBuf, PathBuf, PathBuf), String>) {
    let (net, css_global, css_domains, list_domains) = match paths {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[adblock] disabled: {e}");
            SHARED.with(|s| *s.borrow_mut() = State::Failed);
            return;
        }
    };

    let store_dir = filter_store_dir();
    let _ = std::fs::create_dir_all(&store_dir);

    unsafe {
        let c_dir = std::ffi::CString::new(store_dir.to_string_lossy().as_ref()).unwrap();
        let store = ffi::webkit_user_content_filter_store_new(c_dir.as_ptr());
        // `store` is intentionally leaked — lives for the app lifetime.

        // Compile the JSON rules into the store (persists a binary filter).
        let c_path = std::ffi::CString::new(net.to_string_lossy().as_ref()).unwrap();
        let file = ffi::g_file_new_for_path(c_path.as_ptr());
        let c_id = std::ffi::CString::new(FILTER_ID).unwrap();

        ffi::webkit_user_content_filter_store_save_from_file(
            store,
            c_id.as_ptr(),
            file,
            std::ptr::null_mut(),
            Some(load_trampoline),
            std::ptr::null_mut(),
        );
    }

    // Build cosmetic style sheets (independent of the async compile above).
    unsafe { build_style_sheets(&css_global, &css_domains, &list_domains) };
}

unsafe fn build_style_sheets(
    css_global: &PathBuf,
    css_domains: &PathBuf,
    list_domains: &PathBuf,
) {
    let mut sheets: Vec<*mut c_void> = Vec::new();

    if let Ok(css) = std::fs::read_to_string(css_global) {
        if !css.trim().is_empty() {
            if let Some(s) = make_style_sheet(&css, &[]) {
                sheets.push(s);
            }
        }
    }

    if let (Ok(css), Ok(list)) = (
        std::fs::read_to_string(css_domains),
        std::fs::read_to_string(list_domains),
    ) {
        if !css.trim().is_empty() {
            let globs: Vec<String> = list
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            let c_globs: Vec<std::ffi::CString> =
                globs.iter().map(|g| std::ffi::CString::new(g.as_str()).unwrap()).collect();
            let ptrs: Vec<*const std::os::raw::c_char> =
                c_globs.iter().map(|g| g.as_ptr()).chain(std::iter::once(std::ptr::null())).collect();
            if let Some(s) = make_style_sheet(&css, &ptrs) {
                sheets.push(s);
            }
        }
    }

    STAGED_SHEETS.with(|s| *s.borrow_mut() = sheets);

    STAGED_SCRIPTS.with(|s| *s.borrow_mut() = build_scriptlets());
}

/// JS scriptlets — uBlock-style injected scripts for players that serve ads
/// from the video CDN itself (YouTube), where network rules are useless.
unsafe fn build_scriptlets() -> Vec<*mut c_void> {
    // Runs at document-start on youtube.com: hardens the player against the
    // ad pipeline. The player's ad module is patched to instantly skip any
    // ad playback, and ad UI is removed from the DOM as it appears.
    const YT_AD_KILLER: &str = r#"
(function () {
    if (window.__lbAdKill) return; window.__lbAdKill = true;

    var css = document.createElement('style');
    css.textContent = [
        '.ytp-ad-module,', '.ytp-ad-player-overlay,', '.ytp-ad-text,',
        '.ytp-ad-image,', '.ytp-ad-video,', '.ytp-ad-preview-container',
        '.ytp-ad-action-interstitial,', '.ytp-ad-message-container,',
        'ytd-promoted-sparkles-web-renderer,', 'ytd-promoted-video-renderer,',
        'ytd-compact-promoted-video-renderer,', 'ytd-ad-slot-renderer,',
        '#player-ads,', 'ytd-in-feed-ad-layout-renderer,',
        'ytd-banner-promo-renderer,', 'ytd-statement-banner-renderer,',
        'ytd-rich-item-renderer[data-ad],',
        'tp-yt-paper-dialog.ytd-popup-container ytd-mealbar-promo-renderer'
    ].join(', '), ' { display: none !important; }'].join('');
    (document.head || document.documentElement).appendChild(css);

    function skipAds(player) {
        try {
            // Kill the ad module state machine.
            if (player.getPlayerResponse) {
                var pr = player.getPlayerResponse();
                if (pr && pr.adPlacements) delete pr.adPlacements;
                if (pr && pr.adSlots) delete pr.adSlots;
            }
            // Any adBreak must end immediately.
            if (player.isAdShowing && player.isAdShowing()) {
                if (player.stopVideo) player.stopVideo();
                if (player.seekTo && player.getDuration) {
                    player.seekTo(player.getDuration() || 0, true);
                }
                if (player.closeSlardarAdBreaks) player.closeSlardarAdBreaks();
                if (player.playerResponse && player.playerResponse.adPlacements) {
                    delete player.playerResponse.adPlacements;
                }
                var v = document.querySelector('video.html5-main-video');
                if (v && v.src) { try { v.src = ''; } catch (e) {} }
            }
        } catch (e) {}
    }

    function tick() {
        var player = document.getElementById('movie_player');
        if (player) skipAds(player);
    }

    setInterval(tick, 300);
    document.addEventListener('load', tick, true);
})();
"#;

    let mut scripts = Vec::new();
    let allow = [c"*://www.youtube.com/*", c"*://m.youtube.com/*", c"*://youtube.com/*"];
    let allow_ptrs: Vec<*const std::os::raw::c_char> =
        allow.iter().map(|s| s.as_ptr()).chain(std::iter::once(std::ptr::null())).collect();

    let c_js = std::ffi::CString::new(YT_AD_KILLER).unwrap();
    // 0 = ALL_FRAMES; injection_time 0 = document-start.
    let script = ffi::webkit_user_script_new(
        c_js.as_ptr(),
        0,
        0,
        allow_ptrs.as_ptr(),
        std::ptr::null(),
    );
    if !script.is_null() {
        scripts.push(script);
    }
    scripts
}

thread_local! {
    static STAGED_SHEETS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
    static STAGED_SCRIPTS: RefCell<Vec<*mut c_void>> = const { RefCell::new(Vec::new()) };
}

unsafe fn make_style_sheet(
    css: &str,
    allow_list: &[*const std::os::raw::c_char],
) -> Option<*mut c_void> {
    let c_css = std::ffi::CString::new(css).ok()?;
    let allow_ptr = if allow_list.is_empty() {
        std::ptr::null()
    } else {
        allow_list.as_ptr()
    };
    let sheet = ffi::webkit_user_style_sheet_new(
        c_css.as_ptr(),
        0, // WEBKIT_USER_CONTENT_INJECT_ALL_FRAMES
        0, // WEBKIT_USER_STYLE_LEVEL_USER
        allow_ptr,
        std::ptr::null(),
    );
    if sheet.is_null() {
        None
    } else {
        Some(sheet)
    }
}

unsafe fn finish_load(store: *mut c_void, result: *mut c_void) {
    let mut err: *mut ffi::GError = std::ptr::null_mut();
    let filter =
        ffi::webkit_user_content_filter_store_save_from_file_finish(store, result, &mut err);

    if filter.is_null() {
        let msg = if err.is_null() {
            "unknown error".to_string()
        } else {
            std::ffi::CStr::from_ptr((*err).message)
                .to_string_lossy()
                .into_owned()
        };
        eprintln!("[adblock] filter compile failed: {msg}");
        SHARED.with(|s| {
            let mut st = s.borrow_mut();
            // Cosmetic sheets may still work even if compile failed.
            let sheets = STAGED_SHEETS.with(|s| std::mem::take(&mut *s.borrow_mut()));
            if !sheets.is_empty() {
                *st = State::Ready(FilterBundle {
                    filter: std::ptr::null_mut(),
                    style_sheets: sheets,
                    user_scripts: STAGED_SCRIPTS
                        .with(|s| std::mem::take(&mut *s.borrow_mut())),
                });
            } else {
                *st = State::Failed;
            }
        });
        return;
    }

    let mut bundle = FilterBundle {
        filter,
        style_sheets: Vec::new(),
        user_scripts: STAGED_SCRIPTS.with(|s| std::mem::take(&mut *s.borrow_mut())),
    };

    SHARED.with(|shared| {
        let mut state = shared.borrow_mut();
        match std::mem::replace(&mut *state, State::Failed) {
            State::Loading(pending) => {
                bundle.style_sheets =
                    STAGED_SHEETS.with(|s| std::mem::take(&mut *s.borrow_mut()));
                for manager in &pending {
                    apply_bundle(&bundle, *manager);
                }
                *state = State::Ready(bundle);
            }
            other => {
                *state = other;
            }
        }
    });
    eprintln!("[adblock] EasyList compiled & active (network + cosmetic + scriptlets)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_rules() {
        let data = easylist_to_filters(
            "! comment\n||doubleclick.net^\n/ads/banner.js\n@@||good.example.com^$document\nbannersite.com##.ad\n##.tracker, .pixel\n#@#.keepme\n",
        );
        // Network rules
        assert!(data.network_json.contains(r#""url-filter":"/ads/banner\\.js""#));
        assert!(data.network_json.contains(r#""action":{"type":"block"}"#));
        assert!(data.network_json.contains(r#""action":{"type":"ignore-previous-rules"}"#));
        // `^` becomes a separator class, not a dead anchor
        assert!(data
            .network_json
            .contains(r#""url-filter":"doubleclick\\.net([^a-zA-Z0-9._%+-]|$)""#));
        // Cosmetic rule lands as css-display-none (hide), NEVER as a block —
        // a domain-level block would take out the whole site.
        assert!(data
            .network_json
            .contains(r#""action":{"type":"css-display-none""#));
        assert!(data.network_json.contains(r#""if-domain":["*bannersite.com"]"#));
        assert!(!data
            .network_json
            .contains(r#""action":{"type":"block"},"trigger":{"url-filter":"bannersite"#));
        // Cosmetic selectors landed in the CSS layer
        assert!(data.domain_css.contains(".ad"));
        assert!(data
            .cosmetic_domains
            .iter()
            .any(|g| g.contains("bannersite.com")));
        // Exception hiding rule `#@#` is ignored entirely
        assert!(!data.domain_css.contains(".keepme"));
        // Global selectors
        assert!(data.global_css.contains(".tracker"));
    }

    #[test]
    fn wildcard_and_separator() {
        let data = easylist_to_filters("example.com/ads/*banner\n");
        assert!(data.network_json.contains(r#"ads/.*banner"#));
    }
}

#[cfg(test)]
mod debug_tmp {
    use super::*;
    #[test]
    fn print_json() {
        let d = easylist_to_filters("||doubleclick.net^\n");
        eprintln!("JSON: {}", d.network_json);
    }
}
