const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  // navigation (used by home and by main directly)
  navigate: (input) => ipcRenderer.send('navigate', input),
  goBack: () => ipcRenderer.send('go-back'),
  goForward: () => ipcRenderer.send('go-forward'),
  reload: () => ipcRenderer.send('reload'),
  stop: () => ipcRenderer.send('stop'),
  goHome: () => ipcRenderer.send('go-home'),
  focusView: () => ipcRenderer.send('focus-view'),

  // events for the home page
  onFocusAddressBar: (cb) => ipcRenderer.on('focus-address-bar', cb),
  onUrlChanged: (cb) => ipcRenderer.on('url-changed', (_, url) => cb(url)),
  onViewVisibility: (cb) => ipcRenderer.on('view-visibility', (_, v) => cb(v)),

  // home page requests the command palette (Ctrl+K from home)
  requestShowPalette: () => ipcRenderer.send('show-palette-request'),

  // command palette: shown by main, executes actions and closes
  onPaletteShow: (cb) => ipcRenderer.on('palette-show', cb),
  onPaletteHide: (cb) => ipcRenderer.on('palette-hide', cb),
  paletteAction: (action) => ipcRenderer.send('palette-action', action),
  closePalette: () => ipcRenderer.send('palette-close'),

  // ad blocker: status pushed by main on every change
  onAdblockerUpdate: (cb) => ipcRenderer.on('adblocker-update', (_, status) => cb(status)),
});
