const { app, BrowserWindow, WebContentsView, ipcMain, shell } = require('electron');
const path = require('path');

// --- configurable site list: edit here without touching UI ---
const SITES = [
  { label: 'SahilCodex', url: 'https://sahilcodex.vercel.app' },
  { label: 'GitHub', url: 'https://github.com/sahilcodexx' },
  { label: 'Vercel', url: 'https://vercel.com' },
  { label: 'Bookmrk', url: 'https://bookmrkit.vercel.app' },
  { label: 'KeyUI', url: 'https://keyui.vercel.app' },
  { label: 'TCXCommit', url: 'https://tcxcommit.vercel.app' },
  { label: 'Hacker News', url: 'https://news.ycombinator.com' },
  { label: 'MDN', url: 'https://developer.mozilla.org' },
  { label: 'StackOverflow', url: 'https://stackoverflow.com' },
  { label: 'Next.js', url: 'https://nextjs.org' },
  { label: 'Framer Motion', url: 'https://motion.dev' },
  { label: 'Tailwind', url: 'https://tailwindcss.com' },
  { label: 'Linear', url: 'https://linear.app' },
  { label: 'Raycast', url: 'https://raycast.com' },
  { label: 'Local 3000', url: 'http://localhost:3000' },
];

const SEARCH_ENGINE = 'https://www.google.com/search?q=';
const TOOLBAR_HEIGHT = 0; // floating toolbar, view is full window
let viewVisible = false;

let win;
let view;

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
  // also clear view to blank so back/forward history doesn't leak
  try { view.webContents.loadURL('about:blank'); } catch {}
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

  // toolbar UI lives in the window
  win.loadFile(path.join(__dirname, 'ui', 'index.html'));

  // website lives in WebContentsView - isolated from toolbar
  view = new WebContentsView({
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  win.contentView.addChildView(view);
  // start on home: view hidden, no site autoload
  hideView();
  view.webContents.loadURL('about:blank');
  win.on('resize', updateViewBounds);

  // keep address bar in sync
  const sendUrl = (url) => {
    if (win && !win.isDestroyed()) win.webContents.send('url-changed', url);
  };
  const sendLoading = (isLoading) => {
    if (win && !win.isDestroyed()) win.webContents.send('loading-changed', isLoading);
  };

  view.webContents.on('did-start-loading', () => sendLoading(true));
  view.webContents.on('did-stop-loading', () => sendLoading(false));
  view.webContents.on('did-navigate', (_, url) => {
    if (url !== 'about:blank') showView();
    sendUrl(url);
  });
  view.webContents.on('did-navigate-in-page', (_, url) => sendUrl(url));
  view.webContents.on('did-fail-load', (_, code, desc, url) => {
    // ignore aborted loads
    if (code === -3) return;
    console.error('load failed', code, desc, url);
  });

  // open external protocols in OS browser, keep http(s) inside view
  view.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('http://') || url.startsWith('https://')) {
      view.webContents.loadURL(url);
      return { action: 'deny' };
    }
    shell.openExternal(url);
    return { action: 'deny' };
  });

  // navigation and global shortcuts stay in main.js
  const handleShortcut = (input) => {
    const key = input.key.toLowerCase();
    const ctrl = input.control || input.meta;
    const alt = input.alt;
    const shift = input.shift;

    // Ctrl/Cmd + L -> focus address bar (toolbar owns it, so send to window)
    if (ctrl && key === 'l') {
      if (win && !win.isDestroyed()) {
        win.focus();
        win.webContents.send('focus-address-bar');
      }
      return true;
    }
    // Ctrl/Cmd + R / F5 -> reload
    if ((ctrl && key === 'r') || key === 'f5') {
      view.webContents.reload();
      return true;
    }
    // Ctrl+Shift+I -> DevTools (for view)
    if (ctrl && shift && key === 'i') {
      if (view.webContents.isDevToolsOpened()) view.webContents.closeDevTools();
      else view.webContents.openDevTools({ mode: 'detach' });
      return true;
    }
    // Alt+Left -> back
    if (alt && key === 'arrowleft') {
      if (view.webContents.canGoBack()) view.webContents.goBack();
      return true;
    }
    // Alt+Right -> forward
    if (alt && key === 'arrowright') {
      if (view.webContents.canGoForward()) view.webContents.goForward();
      return true;
    }
    // Esc -> stop
    if (key === 'escape') {
      view.webContents.stop();
      return true;
    }
    return false;
  };

  // capture shortcuts whether focus is in toolbar or website
  win.webContents.on('before-input-event', (event, input) => {
    if (handleShortcut(input)) event.preventDefault();
  });
  view.webContents.on('before-input-event', (event, input) => {
    if (handleShortcut(input)) event.preventDefault();
  });

  win.on('closed', () => {
    win = null;
    view = null;
  });
}

// --- IPC: renderer (toolbar) asks main to navigate ---
ipcMain.handle('get-sites', () => SITES);
ipcMain.handle('get-current-url', () => {
  try { return view?.webContents.getURL() || ''; } catch { return ''; }
});
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
ipcMain.on('show-toolbar', () => {
  if (win && !win.isDestroyed()) win.webContents.send('show-toolbar');
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

// security: deny extra permission requests
app.on('web-contents-created', (_, contents) => {
  contents.on('will-navigate', (e, url) => {
    // allow only http(s) inside view, toolbar is file://
    if (contents === view?.webContents) return;
  });
});
