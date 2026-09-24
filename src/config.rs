//! Minimal browser configuration.

pub const APP_ID: &str = "com.example.LightBrowser";
pub const APP_NAME: &str = "Light Browser";

pub const HOME_PAGE: &str = "https://example.com";

/// Search engine template. The user query is percent-encoded and substituted.
pub const SEARCH_ENGINE_URL: &str = "https://duckduckgo.com/?q={query}";

/// New tab page HTML. Centered search box + suggestions container + search-engine
/// shortcut. Submitting assigns `window.location.href`, which triggers WebKit
/// navigation. The script decides whether the input is a URL or a search query.
pub const NEW_TAB_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>New Tab</title>
<style>
  :root {
    color-scheme: light;
    --bg: #ffffff;
    --bg-elev: #ffffff;
    --fg: #171717;
    --muted: #a6a6a6;
    --line: rgba(0,0,0,0.12);
    --line-soft: rgba(0,0,0,0.06);
    --shadow: 0 18px 40px rgba(0,0,0,0.08), 0 2px 10px rgba(0,0,0,0.04);
  }
  :root.dark, html.dark {
    color-scheme: dark;
    --bg: #141416;
    --bg-elev: #1e1e24;
    --fg: #f5f5f7;
    --muted: #8e8e93;
    --line: rgba(255,255,255,0.12);
    --line-soft: rgba(255,255,255,0.08);
    --shadow: 0 18px 40px rgba(0,0,0,0.40), 0 2px 10px rgba(0,0,0,0.20);
  }
  :root.dark .suggestion strong, html.dark .suggestion strong { color: #f5f5f7; }
  :root.dark .search input::selection, html.dark .search input::selection { background: #2b4c7e; }
  :root.dark .search.focused, html.dark .search.focused { border-color: rgba(255,255,255,0.28); }
  html, body { height: 100%; margin: 0; background: var(--bg); color: var(--fg);
               font-family: -apple-system, system-ui, "Segoe UI", Roboto, "Helvetica Neue",
                            Arial, sans-serif; }
  body { display: flex; flex-direction: column;
         align-items: center; justify-content: center;
         padding: 24px 24px 18vh; box-sizing: border-box; }
  form { width: min(482px, 82vw); display: flex; flex-direction: column; gap: 8px; }

  .search {
    display: flex; flex-direction: row;
    align-items: center; gap: 10px;
    min-height: 41px; padding: 0 15px; border: 1px solid var(--line); border-radius: 9px;
    background: var(--bg-elev); box-shadow: 0 1px 2px rgba(0,0,0,0.035);
    transition: border-color 120ms ease, box-shadow 120ms ease;
  }
  .search.focused {
    border-color: rgba(0,0,0,0.18);
    box-shadow: var(--shadow);
  }
  .search svg { width: 16px; height: 16px; opacity: 0.35; flex: 0 0 16px; }
  .search input {
    flex: 1 1 auto; border: 0; outline: 0; background: transparent; color: var(--fg);
    font-size: 15px; line-height: 1.4; min-width: 0;
  }
  .search input::selection { background: #dce8ff; }

  .suggestions {
    border: 1px solid var(--line-soft); border-radius: 9px; background: var(--bg-elev);
    box-shadow: var(--shadow); overflow: hidden; padding: 5px 0;
  }
  .suggestions:empty { display: none; }
  .suggestion {
    display: flex; align-items: baseline; gap: 8px;
    min-height: 29px; padding: 0 15px; font-size: 12px;
  }
  .suggestion strong { font-weight: 500; color: #151515; }
  .suggestion span { color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .engine {
    color: var(--muted); font-size: 11px;
    display: flex; flex-direction: row; align-items: center; gap: 6px;
    padding: 3px 15px 0;
  }
  .engine svg { width: 13px; height: 13px; }
  .engine .kbd {
    display: inline-flex; align-items: center; justify-content: center;
    min-width: 13px; height: 13px; padding: 0 3px;
    border: 1px solid var(--line); border-radius: 999px;
    font-size: 9px; font-family: ui-monospace, "SF Mono", Menlo, monospace;
    color: var(--fg);
  }
</style>
</head>
<body>
  <form id="f" autocomplete="off">
    <div class="search">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
           stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="7"></circle>
        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
      </svg>
      <input id="q" name="q" autofocus placeholder="Search or enter address"
             spellcheck="false" autocomplete="off">
    </div>
    <div class="suggestions" id="s"></div>
    <div class="engine">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
           stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="7"></circle>
        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
      </svg>
      <span class="kbd">G</span>
      <span>Search with Google</span>
    </div>
  </form>
  <script>
    (function () {
      function applyTheme() {
        if (window.location.hash === '#dark') {
          document.documentElement.classList.add('dark');
        } else if (window.location.hash === '#light') {
          document.documentElement.classList.remove('dark');
        }
      }
      applyTheme();
      window.addEventListener('hashchange', applyTheme);
      var input = document.getElementById('q');
      var form = document.getElementById('f');

      function looksLikeUrl(v) {
        if (/^[a-z][a-z0-9+.-]*:\/\//i.test(v)) return true;
        if (/^localhost(:\d+)?(\/|$)/i.test(v)) return true;
        if (/^\d{1,3}(\.\d{1,3}){3}(:\d+)?(\/|$)/.test(v)) return true;
        if (/^[a-z0-9-]+(\.[a-z0-9-]+)+(:\d+)?(\/|$)/i.test(v)) return true;
        return false;
      }

      function fullUrl(v) {
        if (/^[a-z][a-z0-9+.-]*:\/\//i.test(v)) return v;
        return 'https://' + v;
      }

      function submit() {
        var v = input.value.trim();
        if (!v) return;
        if (looksLikeUrl(v)) {
          window.location.href = fullUrl(v);
        } else {
          window.location.href = 'https://www.google.com/search?q=' + encodeURIComponent(v);
        }
      }

      function titleFor(v) {
        return v.replace(/^https?:\/\//i, '').replace(/\/.*$/, '');
      }

      function renderSuggestions() {
        var v = input.value.trim();
        var box = document.getElementById('s');
        box.innerHTML = '';
        if (!v) return;
        var host = titleFor(v);
        var items = looksLikeUrl(v)
          ? [
              [host, 'Open address'],
              ['www.' + host.replace(/^www\./, ''), 'Website'],
              [host + '/search', 'Search on this site']
            ]
          : [
              [v, 'Google Search'],
              [v + ' docs', 'Search suggestion'],
              [v + ' examples', 'Search suggestion']
            ];
        items.forEach(function (item) {
          var row = document.createElement('div');
          row.className = 'suggestion';
          row.innerHTML = '<strong></strong><span></span>';
          row.querySelector('strong').textContent = item[0];
          row.querySelector('span').textContent = item[1];
          row.addEventListener('mousedown', function (e) {
            e.preventDefault();
            input.value = item[0];
            submit();
          });
          box.appendChild(row);
        });
      }

      form.addEventListener('submit', function (e) { e.preventDefault(); submit(); });
      input.addEventListener('input', renderSuggestions);
      input.addEventListener('focus', function () {
        input.parentElement.classList.add('focused');
      });
      input.addEventListener('blur', function () {
        input.parentElement.classList.remove('focused');
      });
      document.addEventListener('keydown', function (e) {
        if (e.key === 'g' && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey
            && document.activeElement !== input) {
          e.preventDefault();
          input.focus();
        }
      });
    })();
  </script>
</body>
</html>"#;

/// Error page HTML template. `{uri}` and `{message}` placeholders are substituted.
pub const ERROR_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Unable to load page</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 0; padding: 48px;
         background: #fafafa; color: #222; }
  .box { max-width: 640px; margin: 0 auto; background: #fff;
         border: 1px solid #ddd; border-radius: 8px; padding: 32px; }
  h1 { color: #b00; margin-top: 0; }
  .uri { color: #666; word-break: break-all; }
  .msg { background: #f4f4f4; padding: 8px 12px; border-radius: 4px;
         font-family: monospace; font-size: 13px; margin: 16px 0; }
  button { background: #2563eb; color: white; border: none; padding: 8px 16px;
           border-radius: 4px; cursor: pointer; font-size: 14px; }
  button:hover { background: #1d4ed8; }
</style>
</head>
<body>
  <div class="box">
    <h1>Unable to load this page</h1>
    <p class="uri">{uri}</p>
    <p>The webpage could not be loaded.</p>
    <div class="msg">{message}</div>
    <button onclick="window.location.reload()">Try Again</button>
  </div>
</body>
</html>"#;
