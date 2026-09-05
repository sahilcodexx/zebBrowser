const { app, BrowserWindow, WebContentsView, ipcMain, shell, session } = require('electron');
const fs = require('fs');
const https = require('https');
const path = require('path');

const SEARCH_ENGINE = 'https://www.google.com/search?q=';
let viewVisible = false;

let win;
let view;       // site view (full-window when active)
let paletteView; // command palette view (full-window on top of `view` when open)

// --- ad blocker (hand-rolled domain matching + cosmetic CSS injection) ---
// Network-level blocking is a domain-set lookup (parent-domain walk).
// Cosmetic filtering hides DOM-embedded ad slots (e.g. Spotify's
// "Sponsored" row) by injecting CSS on every page load.
const ADBLOCKER_LIST_PATH = path.join(__dirname, 'adblocker', 'lists', 'default.txt');
const ADBLOCKER_UPDATE_URL = 'https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts';
let adBlockerEnabled = true;
let blockedCount = 0;
let adDomains = new Set();
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

// Parse a list of domains. Accepts either bare domains (one per line) or
// hosts format ("127.0.0.1 domain.com" / "0.0.0.0 domain.com"). Skips
// comments, blank lines, and obviously-not-host entries.
function loadAdListFromText(text) {
  const domains = new Set();
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const parts = line.split(/\s+/);
    const candidate = parts[parts.length - 1].toLowerCase();
    if (!candidate) continue;
    if (candidate === 'localhost') continue;
    if (/^\d+\.\d+\.\d+\.\d+$/.test(candidate)) continue; // bare IP
    if (!/^[a-z0-9.-]+$/.test(candidate)) continue;
    if (!candidate.includes('.')) continue;
    domains.add(candidate);
  }
  return domains;
}

function loadAdList() {
  // Prefer a user-saved list (after an Update), fall back to the bundled one.
  const userListPath = path.join(app.getPath('userData'), 'adblocker-list.txt');
  try {
    const text = fs.readFileSync(userListPath, 'utf8');
    adDomains = loadAdListFromText(text);
    return;
  } catch {}
  try {
    const text = fs.readFileSync(ADBLOCKER_LIST_PATH, 'utf8');
    adDomains = loadAdListFromText(text);
  } catch (err) {
    console.error('Failed to load ad list:', err);
    adDomains = new Set();
  }
}

function isAdUrl(url) {
  if (!adBlockerEnabled) return false;
  let hostname;
  try {
    hostname = new URL(url).hostname.toLowerCase();
  } catch {
    return false;
  }
  // Walk parent domains: cdn.example.com -> example.com -> com (skip com)
  const parts = hostname.split('.');
  for (let i = 0; i < parts.length - 1; i++) {
    const domain = parts.slice(i).join('.');
    if (adDomains.has(domain)) return true;
  }
  return false;
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

function setupAdBlocker() {
  loadAdblockerConfig();
  loadAdList();

  try {
    session.defaultSession.webRequest.onBeforeRequest({
      urls: ['http://*/*', 'https://*/*']
    }, (details, callback) => {
      if (isAdUrl(details.url)) {
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

function fetchAdListFromUrl(urlString) {
  return new Promise((resolve, reject) => {
    const req = https.get(urlString, { headers: { 'User-Agent': 'MiniBrowser/1.3' } }, (res) => {
      // Follow one level of redirect
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        fetchAdListFromUrl(res.headers.location).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode}`));
        return;
      }
      let data = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => resolve(data));
    });
    req.on('error', reject);
    req.setTimeout(15000, () => { req.destroy(new Error('Request timed out')); });
  });
}

async function updateAdList() {
  try {
    const text = await fetchAdListFromUrl(ADBLOCKER_UPDATE_URL);
    const newDomains = loadAdListFromText(text);
    if (newDomains.size === 0) {
      return { success: false, error: 'Parsed list is empty' };
    }
    adDomains = newDomains;
    blockedCount = 0;
    try {
      fs.writeFileSync(path.join(app.getPath('userData'), 'adblocker-list.txt'), text, 'utf8');
    } catch (err) {
      console.error('Failed to persist ad list:', err);
    }
    sendAdblockerUpdate();
    return { success: true, count: adDomains.size };
  } catch (err) {
    return { success: false, error: err.message };
  }
}

// Cosmetic filtering: on every page load, inject CSS that hides the
// common ad-slot patterns (e.g. Spotify's "Sponsored" row, YouTube's
// "Sponsored" overlay). This is the part network blocking alone cannot
// touch — the ad slot is the site's own DOM, so we need to hide it in
// CSS. The selector list is intentionally conservative; users can
// toggle the ad blocker off if it ever hides something legitimate.
//
// KNOWN LIMITATION — YouTube / YouTube Music video ads: the ad video
// stream comes from googlevideo.com (the same CDN as content) and
// plays in YouTube's own player element. We can hide the "Sponsored"
// overlay and ad-slot renderers, but the ad video itself will still
// play. For a truly ad-free YouTube Music experience, YouTube Music
// Premium is required; a YouTube-player-hook (player.seekTo to skip
// the ad) is possible but fragile.
const ADBLOCKER_COSMETIC_CSS = `
  /* generic */
  .ad, .ads, .ad-container, .ad-banner, .ad-wrapper, .ad-slot,
  .ad-placement, .ad-placement-slug, .ad-unit, .ad-zone,
  .advert, .advertisement, .advertising, .advertising-container,
  .sponsored, .promoted, .promotion, .sponsor,
  [data-ad], [data-ad-slot], [data-adunit], [data-advertisement],
  [data-testid="ad"], [aria-label="advertisement"],
  [class*="Ad-"], [class*="Ads-"], [class*="Sponsored-"],

  /* YouTube / YouTube Music — hides Sponsored overlay, ad-slot
     renderers, and the image-overlay banners. The ad video in the
     main player will still play (see KNOWN LIMITATION above). */
  ytd-ad-slot-renderer,
  ytmusic-ad-slot-renderer,
  ytmusic-ad-renderer,
  .ytp-ad-text-overlay,
  .ytp-ad-overlay-container,
  .ytp-ad-image-overlay,
  .ytp-ad-overlay-close-button,
  .ytp-ad-persistent-progress-bar-container,
  #player-ads,
  #masthead-ad,
  ytd-promoted-video-renderer,
  ytd-display-ad-renderer,
  ytd-in-feed-ad-layout-renderer,
  ytd-rich-item-renderer:has(ytd-ad-slot-renderer)
{ display: none !important; }
`;

function setupCosmeticFiltering() {
  if (!view) return;
  view.webContents.on('dom-ready', () => {
    if (!adBlockerEnabled) return;
    const url = view.webContents.getURL();
    if (!url || url === 'about:blank' || url.startsWith('file://')) return;
    view.webContents.insertCSS(ADBLOCKER_COSMETIC_CSS).catch(() => {});
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
      updateAdList();
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
