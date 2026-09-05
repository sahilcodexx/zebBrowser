const { app, BrowserWindow, WebContentsView, ipcMain, shell, session } = require('electron');
const fs = require('fs');
const https = require('https');
const path = require('path');

const SEARCH_ENGINE = 'https://www.google.com/search?q=';
let viewVisible = false;

let win;
let view;       // site view (full-window when active)
let paletteView; // command palette view (full-window on top of `view` when open)

// --- ad blocker (uBlock Origin-compatible engine via @ghostery/adblocker) ---
const { FiltersEngine, Request } = require('@ghostery/adblocker');
const ADBLOCKER_LIST_URLS = [
  'https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/filters.txt',
  'https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/privacy.txt',
];
let adBlockerEnabled = true;
let blockedCount = 0;
let adEngine = null;        // FiltersEngine instance
let adEngineReady = false;  // true once init/update completes
let lastAdblockerBroadcast = 0;

function isUrl(text) {
  const t = text.trim();
  if (/^https?:\/\//i.test(t)) return true;
  if (/^localhost(:\d+)?(\/|$)/i.test(t)) return true;
  if (/^[a-z0-9.-]+\.[a-z]{2,}(\/|$)/i.test(t) && !/\s/.test(t)) return true;
  return false;
}

function toUrl(input) {
  const t = input.trim();
  if (!t) return null;
  if (/^https?:\/\//i.test(t)) return t;
  if (/^localhost(:\d+)?(\/|$)/i.test(t)) return `http://${t}`;
  if (isUrl(t)) return `https://${t}`;
  return SEARCH_ENGINE + encodeURIComponent(t);
}

function updateViewBounds() {
  if (!win || !view) return;
  const bounds = win.getContentBounds();
  if (!viewVisible) {
    view.setBounds({ x: 0, y: 0, width: 0, height: 0 });
  } else {
    view.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height });
  }
}

function showView() {
  viewVisible = true;
  updateViewBounds();
  if (win && !win.isDestroyed()) win.webContents.send('view-visibility', true);
}

function hideView() {
  viewVisible = false;
  updateViewBounds();
  if (win && !win.isDestroyed()) win.webContents.send('view-visibility', false);
  try { view.webContents.loadURL('about:blank'); } catch {}
}

// ---------- ad blocker ----------

// Config persistence: adBlockerEnabled (on/off) lives in userData/config.json.
function loadAdblockerConfig() {
  const configPath = path.join(app.getPath('userData'), 'config.json');
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    adBlockerEnabled = config.adBlockerEnabled !== false;
  } catch {
    adBlockerEnabled = true; // default ON
  }
}

function saveAdblockerConfig() {
  const configPath = path.join(app.getPath('userData'), 'config.json');
  try {
    fs.writeFileSync(configPath, JSON.stringify({ adBlockerEnabled }, null, 2));
  } catch (err) {
    console.error('Failed to save ad blocker config:', err);
  }
}

// Broadcast {enabled, blockedCount} to the home and palette webContents,
// throttled to <=1/400ms so a busy page with many blocked requests does
// not flood the renderer.
function sendAdblockerUpdate() {
  const now = Date.now();
  if (blockedCount > 1 && now - lastAdblockerBroadcast < 400) return;
  lastAdblockerBroadcast = now;

  const status = { enabled: adBlockerEnabled, blockedCount };
  const send = (wc) => {
    if (wc && !wc.isDestroyed()) wc.send('adblocker-update', status);
  };
  send(win && win.webContents);
  send(paletteView && paletteView.webContents);
}

function adEngineCachePath() {
  return path.join(app.getPath('userData'), 'adblocker-engine.bin');
}

// Persist the engine to disk so subsequent startups skip list parsing.
function persistAdEngine() {
  if (!adEngine) return;
  try {
    const buf = Buffer.from(adEngine.serialize());
    fs.writeFileSync(adEngineCachePath(), buf);
  } catch (err) {
    console.error('Failed to persist ad blocker engine:', err);
  }
}

// Load the engine. Tries the on-disk cache first, then falls back to the
// bundled prebuilt engine (ads + tracking, no network). Both are uBO-
// compatible filter lists maintained by the @ghostery/adblocker project.
async function initAdEngine() {
  // 1) Disk cache (fastest, no network)
  try {
    const buf = fs.readFileSync(adEngineCachePath());
    adEngine = FiltersEngine.deserialize(new Uint8Array(buf));
    adEngineReady = true;
    return;
  } catch {}

  // 2) Prebuilt engine (bundled, no network)
  try {
    adEngine = await FiltersEngine.fromPrebuiltAdsAndTracking();
    adEngineReady = true;
    persistAdEngine();
  } catch (err) {
    console.error('Failed to initialize ad blocker engine:', err);
    adEngine = null;
  }
}

// Fetch the latest uBO filter lists and rebuild the engine. Used by the
// "Update Ad Blocker List" palette command.
async function updateAdEngineFromLists() {
  try {
    adEngine = await FiltersEngine.fromLists(fetch, ADBLOCKER_LIST_URLS);
    adEngineReady = true;
    blockedCount = 0;
    persistAdEngine();
    sendAdblockerUpdate();
    return { success: true, lists: adEngine.loadedLists() };
  } catch (err) {
    return { success: false, error: err.message };
  }
}

// Map Electron's webRequest resourceType to @ghostery/adblocker's type names.
function mapElectronResourceType(t) {
  switch (t) {
    case 'mainFrame':
    case 'subFrame':     return 'document';
    case 'stylesheet':   return 'stylesheet';
    case 'script':       return 'script';
    case 'image':        return 'image';
    case 'font':         return 'font';
    case 'object':       return 'object';
    case 'xhr':          return 'xhr';
    case 'ping':         return 'ping';
    case 'cspReport':    return 'csp_report';
    case 'media':        return 'media';
    case 'webSocket':    return 'websocket';
    default:             return 'other';
  }
}

function isAdRequest(details) {
  if (!adBlockerEnabled || !adEngine || !adEngineReady) return false;
  try {
    const sourceUrl = (details.documentURL && details.documentURL !== 'about:blank')
      ? details.documentURL
      : (details.frame && details.frame.url) || details.url;
    const request = Request.fromRawDetails({
      url: details.url,
      sourceUrl,
      type: mapElectronResourceType(details.resourceType),
    });
    return adEngine.match(request) !== null;
  } catch {
    return false;
  }
}

function setupAdBlocker() {
  loadAdblockerConfig();

  // Fire-and-forget: init the engine in the background. The webRequest
  // listener below is registered immediately but only blocks once the
  // engine is ready.
  initAdEngine().catch((err) => console.error('Ad engine init failed:', err));

  try {
    session.defaultSession.webRequest.onBeforeRequest({
      urls: ['http://*/*', 'https://*/*'],
    }, (details, callback) => {
      if (isAdRequest(details)) {
        blockedCount++;
        sendAdblockerUpdate();
        callback({ cancel: true });
      } else {
        callback({ cancel: false });
      }
    });
  } catch (err) {
    console.error('Failed to register ad blocker webRequest listener:', err);
  }
}

// Cosmetic filtering: on each page load, query the page for its classes
// and ids, ask the engine for matching CSS, and inject it before the
// browser paints. This is what hides ad slots that are embedded in the
// page's own DOM (e.g. Spotify's "Sponsored" row) — network blocking
// alone can't do that.
function setupCosmeticFiltering() {
  if (!view) return;
  view.webContents.on('dom-ready', async () => {
    if (!adBlockerEnabled || !adEngine || !adEngineReady) return;
    const url = view.webContents.getURL();
    if (!url || url === 'about:blank' || url.startsWith('file://')) return;

    let hostname;
    try { hostname = new URL(url).hostname; } catch { return; }

    let payload;
    try {
      payload = await view.webContents.executeJavaScript(`(() => {
        const els = document.querySelectorAll('*');
        const classes = new Set();
        for (const el of els) {
          if (el.classList && el.classList.length) {
            for (const c of el.classList) {
              if (c && c.length <= 100) classes.add(c);
              if (classes.size >= 500) break;
            }
          }
          if (classes.size >= 500) break;
        }
        const idEls = document.querySelectorAll('[id]');
        const ids = new Set();
        for (const el of idEls) {
          if (el.id) ids.add(el.id);
          if (ids.size >= 500) break;
        }
        return JSON.stringify({ classes: [...classes], ids: [...ids] });
      })()`);
    } catch {
      return;
    }
    if (!payload) return;

    let classes, ids;
    try { ({ classes, ids } = JSON.parse(payload)); } catch { return; }

    let cosmetic;
    try {
      cosmetic = adEngine.getCosmeticsFilters({ url, hostname, classes, ids });
    } catch {
      return;
    }
    if (cosmetic && cosmetic.stylesheet) {
      try {
        await view.webContents.insertCSS(cosmetic.stylesheet);
      } catch {}
    }
  });
}

// ---------- command palette ----------

function showPalette() {
  if (!win || !paletteView) return;
  const bounds = win.getContentBounds();
  paletteView.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height });
  paletteView.webContents.focus();
  // Send the current ad blocker state so the palette renders the right label/count.
  paletteView.webContents.send('adblocker-update', { enabled: adBlockerEnabled, blockedCount });
  paletteView.webContents.send('palette-show');
}

function hidePalette() {
  if (!paletteView) return;
  paletteView.setBounds({ x: 0, y: 0, width: 0, height: 0 });
  paletteView.webContents.send('palette-hide');
  // restore focus: view if a site is showing, otherwise the home page + search
  if (viewVisible) {
    view?.webContents.focus();
  } else {
    win?.webContents.focus();
    win?.webContents.send('focus-address-bar');
  }
}

function createWindow() {
  win = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    backgroundColor: '#ffffff',
    icon: path.join(__dirname, 'build', 'icon.png'),
    titleBarStyle: 'hiddenInset',
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  // home page lives in the BrowserWindow's webContents (bottom of the stack)
  win.loadFile(path.join(__dirname, 'ui', 'index.html'));
  win.once('ready-to-show', () => {
    win.focus();
    win.webContents.focus();
  });
  win.webContents.on('did-finish-load', () => {
    win.webContents.focus();
    win.webContents.send('focus-address-bar');
  });

  // site view (middle of the stack)
  view = new WebContentsView({
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  win.contentView.addChildView(view);
  hideView();
  view.webContents.loadURL('about:blank');

  // Cosmetic filtering: on every page load, inject CSS that hides ad
  // elements (e.g. Spotify's "Sponsored" row). Network blocking alone
  // can't do this — the ad slot is the site's own DOM.
  setupCosmeticFiltering();

  // palette view (top of the stack) — added AFTER the site view so it renders above it
  paletteView = new WebContentsView({
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  paletteView.setBounds({ x: 0, y: 0, width: 0, height: 0 });
  // transparent so the site stays visible behind the dimmed backdrop
  if (typeof paletteView.setBackgroundColor === 'function') {
    paletteView.setBackgroundColor('#00000000');
  }
  paletteView.webContents.loadFile(path.join(__dirname, 'ui', 'palette.html'));
  paletteView.webContents.on('did-finish-load', () => {
    if (typeof paletteView.setBackgroundColor === 'function') {
      paletteView.setBackgroundColor('#00000000');
    }
  });
  win.contentView.addChildView(paletteView);

  // keep renderer in sync with the current URL
  const sendUrl = (url) => {
    if (win && !win.isDestroyed()) win.webContents.send('url-changed', url);
  };

  view.webContents.on('did-navigate', (_, url) => {
    if (url !== 'about:blank') showView();
    sendUrl(url);
  });
  view.webContents.on('did-navigate-in-page', (_, url) => sendUrl(url));
  view.webContents.on('did-fail-load', (_, code, desc, url) => {
    if (code === -3) return;
    console.error('load failed', code, desc, url);
  });

  view.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('http://') || url.startsWith('https://')) {
      view.webContents.loadURL(url);
      return { action: 'deny' };
    }
    shell.openExternal(url);
    return { action: 'deny' };
  });

  // keep both child views sized to the window
  win.on('resize', () => {
    const bounds = win.getContentBounds();
    if (viewVisible) {
      view.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height });
    }
    if (paletteView.getBounds().width > 0) {
      paletteView.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height });
    }
  });

  // global shortcuts — only the view is wired here.
  // The win/palette handle their own keys so the palette can own Esc/Enter/↑↓.
  const handleShortcut = (input) => {
    const key = input.key.toLowerCase();
    const ctrl = input.control || input.meta;
    const alt = input.alt;
    const shift = input.shift;

    if (ctrl && key === 'l') {
      if (win && !win.isDestroyed()) {
        win.focus();
        win.webContents.send('focus-address-bar');
      }
      return true;
    }
    if (ctrl && key === 'd') {
      if (win && !win.isDestroyed()) {
        win.focus();
        showPalette();
      }
      return true;
    }
    if ((ctrl && key === 'r') || key === 'f5') {
      view.webContents.reload();
      return true;
    }
    if (ctrl && shift && key === 'i') {
      if (view.webContents.isDevToolsOpened()) view.webContents.closeDevTools();
      else view.webContents.openDevTools({ mode: 'detach' });
      return true;
    }
    if (alt && key === 'arrowleft') {
      if (view.webContents.canGoBack()) view.webContents.goBack();
      return true;
    }
    if (alt && key === 'arrowright') {
      if (view.webContents.canGoForward()) view.webContents.goForward();
      return true;
    }
    if (key === 'escape') {
      view.webContents.stop();
      return true;
    }
    return false;
  };

  view.webContents.on('before-input-event', (event, input) => {
    if (handleShortcut(input)) event.preventDefault();
  });
  win.webContents.on('before-input-event', (event, input) => {
    const key = input.key.toLowerCase();
    const ctrl = input.control || input.meta;
    if (ctrl && key === 'd') {
      showPalette();
      event.preventDefault();
    }
  });
  paletteView.webContents.on('before-input-event', (event, input) => {
    if (input.key.toLowerCase() === 'escape') {
      hidePalette();
      event.preventDefault();
    }
  });

  win.on('closed', () => {
    win = null;
    view = null;
    paletteView = null;
  });
}

// --- IPC: home (toolbar) asks main to navigate ---
ipcMain.on('navigate', (_, input) => {
  const url = toUrl(input);
  if (url && view) {
    showView();
    view.webContents.loadURL(url);
  }
});
ipcMain.on('go-home', () => hideView());
ipcMain.on('go-back', () => {
  if (view?.webContents.canGoBack()) view.webContents.goBack();
  else hideView();
});
ipcMain.on('go-forward', () => { if (view?.webContents.canGoForward()) view.webContents.goForward(); });
ipcMain.on('reload', () => view?.webContents.reload());
ipcMain.on('stop', () => view?.webContents.stop());
ipcMain.on('focus-view', () => view?.webContents.focus());

// --- IPC: palette plumbing ---
ipcMain.on('show-palette-request', () => showPalette());
ipcMain.on('palette-close', () => hidePalette());
ipcMain.on('palette-action', (_, action) => {
  hidePalette();
  if (!action || typeof action !== 'object') return;
  switch (action.type) {
    case 'go-home':
      hideView();
      break;
    case 'go-back':
      if (view?.webContents.canGoBack()) view.webContents.goBack();
      else hideView();
      break;
    case 'go-forward':
      if (view?.webContents.canGoForward()) view.webContents.goForward();
      break;
    case 'reload':
      view?.webContents.reload();
      break;
    case 'navigate':
      if (action.query) {
        const url = toUrl(action.query);
        if (url && view) {
          showView();
          view.webContents.loadURL(url);
        }
      }
      break;
    case 'adblocker-toggle':
      adBlockerEnabled = !adBlockerEnabled;
      saveAdblockerConfig();
      sendAdblockerUpdate();
      break;
    case 'adblocker-update':
      // Fire-and-forget; the result arrives via 'adblocker-update' to the palette.
      updateAdEngineFromLists();
      break;
  }
});

app.whenReady().then(() => {
  setupAdBlocker();
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('web-contents-created', (_, contents) => {
  contents.on('will-navigate', (e, url) => {
    if (contents === view?.webContents) return;
  });
});
