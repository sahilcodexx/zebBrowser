// Mini Browser renderer — home (centered search input) only.
// The command palette lives in a separate WebContentsView (ui/palette.html)
// and is requested via 'show-palette-request' on Ctrl+D from this page.

const search = document.getElementById('search');
const searchForm = document.getElementById('search-form');
const home = document.getElementById('home');

let viewHasContent = false; // true while a website is loaded in the WebContentsView

function showHome(show) {
  home.classList.toggle('hidden', !show);
  viewHasContent = !show;
  if (show) {
    setTimeout(() => { search.focus(); search.select(); }, 10);
    // extra focus attempt once window is ready (fixes “need to click” on cold start)
    requestAnimationFrame(() => { search.focus(); search.select(); });
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
  search.value = '';
  showHome(true);
  window.electronAPI.goHome();
}

searchForm.addEventListener('submit', (e) => {
  e.preventDefault();
  navigate(search.value);
});

window.electronAPI.onFocusAddressBar(() => {
  if (viewHasContent) {
    goHome();
  } else {
    search.focus();
    search.select();
  }
});

window.electronAPI.onUrlChanged((url) => {
  if (!url || url === 'about:blank' || url.startsWith('file://')) {
    if (!viewHasContent) return;
    showHome(true);
    search.value = '';
    return;
  }
  showHome(false);
});

window.electronAPI.onViewVisibility((visible) => {
  if (visible) {
    home.classList.add('hidden');
    viewHasContent = true;
  } else {
    home.classList.remove('hidden');
    viewHasContent = false;
    search.value = '';
  }
});

// global shortcuts while the renderer has focus (i.e. on the home page)
window.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault();
    if (viewHasContent) {
      goHome();
    } else {
      search.focus();
      search.select();
    }
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
    e.preventDefault();
    window.electronAPI.requestShowPalette();
  }
  if (e.key === 'Escape' && viewHasContent) {
    e.preventDefault();
    goHome();
  }
});

// autofocus: typing on home should jump straight into the search box
document.addEventListener('keydown', (e) => {
  if (viewHasContent) return;
  if (document.activeElement === search) return;
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key.length === 1 && !e.defaultPrevented) {
    search.focus();
  }
});
window.addEventListener('focus', () => {
  if (!viewHasContent) search.focus();
});

// initial state: home visible, search focused
showHome(true);
