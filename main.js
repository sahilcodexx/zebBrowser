const { app, BrowserWindow, WebContentsView, ipcMain, shell } = require('electron');
const path = require('path');

const SEARCH_ENGINE = 'https://www.google.com/search?q=';
let viewVisible = false;

let win;
let view;       // site view (full-window when active)
let paletteView; // command palette view (full-window on top of `view` when open)

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

// ---------- command palette ----------

function showPalette() {
  if (!win || !paletteView) return;
  const bounds = win.getContentBounds();
  paletteView.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height });
  paletteView.webContents.focus();
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
  }
});

app.whenReady().then(() => {
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
