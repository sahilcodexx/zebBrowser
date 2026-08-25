const address = document.getElementById('address');
const backBtn = document.getElementById('back');
const forwardBtn = document.getElementById('forward');
const reloadBtn = document.getElementById('reload');
const stopBtn = document.getElementById('stop');
const homeBtn = document.getElementById('homeBtn');
const loadingEl = document.getElementById('loading');
const home = document.getElementById('home');
const sitesEl = document.getElementById('sites');
const toolbar = document.getElementById('toolbar');
const hoverTrigger = document.getElementById('hover-trigger');

let isLoading = false;
let viewHasContent = false;

function setLoading(v) {
  isLoading = v;
  loadingEl.classList.toggle('hidden', !v);
  stopBtn.classList.toggle('hidden', !v);
  reloadBtn.classList.toggle('hidden', v);
}

function showHome(show) {
  home.classList.toggle('hidden', !show);
  // Zen: on home, toolbar always visible; on site, auto-hide
  if (show) {
    toolbar.classList.add('visible');
    viewHasContent = false;
  } else {
    toolbar.classList.remove('visible');
    viewHasContent = true;
  }
}

function navigate(value) {
  const v = value.trim();
  if (!v) return;
  window.electronAPI.navigate(v);
  showHome(false);
  setTimeout(() => window.electronAPI.focusView(), 80);
}

function goHome() {
  address.value = '';
  showHome(true);
  address.focus();
  window.electronAPI.goHome();
}

// toolbar events -> main.js
homeBtn.addEventListener('click', goHome);
backBtn.addEventListener('click', () => window.electronAPI.goBack());
forwardBtn.addEventListener('click', () => window.electronAPI.goForward());
reloadBtn.addEventListener('click', () => window.electronAPI.reload());
stopBtn.addEventListener('click', () => window.electronAPI.stop());

address.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    navigate(address.value);
  }
  if (e.key === 'Escape') {
    e.preventDefault();
    // just close bar, keep current page
    address.blur();
    if (viewHasContent) {
      toolbar.classList.remove('visible');
      window.electronAPI.focusView();
    }
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault();
    address.focus(); address.select();
  }
});

address.addEventListener('focus', () => {
  toolbar.classList.add('visible');
  address.select();
});
address.addEventListener('blur', () => {
  // keep visible if home, else allow auto-hide after delay
  if (viewHasContent) {
    setTimeout(() => toolbar.classList.remove('visible'), 400);
  }
});

// hover trigger keeps toolbar visible
hoverTrigger.addEventListener('mouseenter', () => toolbar.classList.add('visible'));
toolbar.addEventListener('mouseleave', () => {
  if (viewHasContent && document.activeElement !== address) {
    toolbar.classList.remove('visible');
  }
});

// main -> renderer - Ctrl+L should always show floating bar over current site
window.electronAPI.onFocusAddressBar(() => {
  // don't toggle home - stay where you are, just show bar
  toolbar.classList.add('visible');
  // keep home state as is: if on home, keep home visible; if on site, keep site
  address.focus();
  // small delay to ensure focus before select (Wayland focus async)
  setTimeout(() => { address.focus(); address.select(); }, 10);
});

window.electronAPI.onUrlChanged((url) => {
  if (!url || url === 'about:blank' || url.startsWith('file://')) {
    // don't overwrite if home
    if (!viewHasContent) return;
    // if navigated back to blank, show home
    showHome(true);
    address.value = '';
    return;
  }
  address.value = url;
  showHome(false);
});

window.electronAPI.onLoadingChanged((v) => setLoading(v));
window.electronAPI.onViewVisibility((visible) => {
  if (visible) {
    home.classList.add('hidden');
    toolbar.classList.remove('visible');
    viewHasContent = true;
  } else {
    home.classList.remove('hidden');
    toolbar.classList.add('visible');
    viewHasContent = false;
    address.value = '';
    setLoading(false);
  }
});
window.electronAPI.onShowToolbar(() => toolbar.classList.add('visible'));

// build site shortcuts from main.js SITES array
(async () => {
  const sites = await window.electronAPI.getSites();
  sitesEl.innerHTML = '';
  for (const s of sites) {
    const btn = document.createElement('button');
    btn.className = 'site';
    btn.innerHTML = `${s.label}<span>${s.url}</span>`;
    btn.title = s.url;
    btn.addEventListener('click', () => {
      address.value = s.url;
      navigate(s.url);
    });
    sitesEl.appendChild(btn);
  }
})();

// keyboard shortcuts while toolbar focused
window.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r') {
    e.preventDefault(); window.electronAPI.reload();
  }
  if (e.key === 'F5') { e.preventDefault(); window.electronAPI.reload(); }
  if (e.altKey && e.key === 'ArrowLeft') { e.preventDefault(); window.electronAPI.goBack(); }
  if (e.altKey && e.key === 'ArrowRight') { e.preventDefault(); window.electronAPI.goForward(); }
  if (e.key === 'Escape' && isLoading) { e.preventDefault(); window.electronAPI.stop(); }
});

// initial state: home visible, toolbar pinned
showHome(true);
