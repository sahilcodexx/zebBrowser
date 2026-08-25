const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  getSites: () => ipcRenderer.invoke('get-sites'),
  getCurrentUrl: () => ipcRenderer.invoke('get-current-url'),
  navigate: (input) => ipcRenderer.send('navigate', input),
  goBack: () => ipcRenderer.send('go-back'),
  goForward: () => ipcRenderer.send('go-forward'),
  reload: () => ipcRenderer.send('reload'),
  stop: () => ipcRenderer.send('stop'),
  goHome: () => ipcRenderer.send('go-home'),
  focusView: () => ipcRenderer.send('focus-view'),
  showToolbar: () => ipcRenderer.send('show-toolbar'),
  onFocusAddressBar: (cb) => ipcRenderer.on('focus-address-bar', cb),
  onUrlChanged: (cb) => ipcRenderer.on('url-changed', (_, url) => cb(url)),
  onLoadingChanged: (cb) => ipcRenderer.on('loading-changed', (_, v) => cb(v)),
  onViewVisibility: (cb) => ipcRenderer.on('view-visibility', (_, v) => cb(v)),
  onShowToolbar: (cb) => ipcRenderer.on('show-toolbar', cb),
});
