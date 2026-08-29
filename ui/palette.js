// Command palette — runs in its own WebContentsView on top of the site view.
// All navigation actions are sent to main via `paletteAction`; main handles
// the actual `navigate`/`go-back`/etc. and closes this view.

const palette = document.getElementById('palette');
const paletteBackdrop = document.getElementById('palette-backdrop');
const paletteInput = document.getElementById('palette-input');
const paletteResults = document.getElementById('palette-results');

let isOpen = false;
let selectedIndex = 0;
let commands = [];

const STATIC_COMMANDS = [
  { id: 'go-home',    label: 'Go Home',      icon: '⌂', shortcut: 'Ctrl+H', section: 'Navigation', keywords: ['home'] },
  { id: 'go-back',    label: 'Go Back',      icon: '←', shortcut: 'Alt+←',  section: 'Navigation', keywords: ['back', 'previous'] },
  { id: 'go-forward', label: 'Go Forward',   icon: '→', shortcut: 'Alt+→',  section: 'Navigation', keywords: ['forward', 'next'] },
  { id: 'reload',     label: 'Reload Page',  icon: '↻', shortcut: 'Ctrl+R', section: 'Navigation', keywords: ['reload', 'refresh'] },
];

function looksLikeUrl(text) {
  const t = text.trim();
  if (/^https?:\/\//i.test(t)) return true;
  if (/^localhost(:\d+)?(\/|$)/i.test(t)) return true;
  if (/^[a-z0-9.-]+\.[a-z]{2,}(\/|$)/i.test(t) && !/\s/.test(t)) return true;
  return false;
}

function getCommands(query) {
  const raw = query.trim();
  const q = raw.toLowerCase();
  const cmds = [];

  if (raw) {
    const isUrl = looksLikeUrl(raw);
    cmds.push({
      id: 'navigate',
      label: isUrl ? `Go to "${raw}"` : `Search for "${raw}"`,
      icon: isUrl ? '→' : '⌕',
      section: 'Action',
      query: raw,
      shortcut: '↵',
    });
  }

  for (const cmd of STATIC_COMMANDS) {
    if (!q || cmd.label.toLowerCase().includes(q) || cmd.keywords.some(k => k.includes(q))) {
      cmds.push(cmd);
    }
  }

  return cmds;
}

function escapeHtml(text) {
  return String(text)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function renderPalette() {
  commands = getCommands(paletteInput.value);
  if (selectedIndex >= commands.length) {
    selectedIndex = Math.max(0, commands.length - 1);
  }

  const sections = [];
  const byName = new Map();
  for (const cmd of commands) {
    if (!byName.has(cmd.section)) {
      const entry = { name: cmd.section, items: [] };
      byName.set(cmd.section, entry);
      sections.push(entry);
    }
    byName.get(cmd.section).items.push(cmd);
  }

  let html = '';
  let globalIndex = 0;
  for (const section of sections) {
    html += `<div class="palette-section">`;
    html += `<div class="palette-section-title">${escapeHtml(section.name)}</div>`;
    for (const item of section.items) {
      const isSelected = globalIndex === selectedIndex;
      const classes = ['palette-item'];
      if (isSelected) classes.push('selected');
      html += `<button type="button" class="${classes.join(' ')}" data-index="${globalIndex}" data-id="${escapeHtml(item.id)}">`;
      html += `<span class="palette-item-icon" aria-hidden="true">${escapeHtml(item.icon)}</span>`;
      html += `<span class="palette-item-label">${escapeHtml(item.label)}</span>`;
      if (item.shortcut) {
        html += `<span class="palette-item-shortcut">${escapeHtml(item.shortcut)}</span>`;
      }
      html += `</button>`;
      globalIndex++;
    }
    html += `</div>`;
  }

  if (commands.length === 0) {
    html = `<div class="palette-section"><div class="palette-section-title">No matches</div></div>`;
  }

  paletteResults.innerHTML = html;
}

function show() {
  if (isOpen) return;
  isOpen = true;
  selectedIndex = 0;
  paletteInput.value = '';
  palette.classList.remove('hidden');
  renderPalette();
  // focus after the palette is in the DOM and visible — double-tick for reliability
  requestAnimationFrame(() => paletteInput.focus());
  setTimeout(() => { paletteInput.focus(); paletteInput.select(); }, 10);
  setTimeout(() => { paletteInput.focus(); }, 50);
}

function close() {
  if (!isOpen) return;
  isOpen = false;
  palette.classList.add('hidden');
  window.electronAPI.closePalette();
}

function moveSelection(delta) {
  if (commands.length === 0) return;
  const n = commands.length;
  selectedIndex = (selectedIndex + delta + n) % n;
  renderPalette();
  const selected = paletteResults.querySelector('.palette-item.selected');
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

function executeCommand(cmd) {
  if (!cmd) return;
  // tell main to execute the action and close us
  if (cmd.id === 'navigate') {
    window.electronAPI.paletteAction({ type: 'navigate', query: cmd.query });
  } else {
    window.electronAPI.paletteAction({ type: cmd.id });
  }
}

paletteInput.addEventListener('input', () => {
  selectedIndex = 0;
  renderPalette();
});

paletteInput.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    close();
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    moveSelection(1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    moveSelection(-1);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    executeCommand(commands[selectedIndex]);
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'd') {
    e.preventDefault();
    e.stopPropagation();
    close();
  }
});

paletteResults.addEventListener('click', (e) => {
  const item = e.target.closest('.palette-item');
  if (!item) return;
  const index = parseInt(item.dataset.index, 10);
  executeCommand(commands[index]);
});

paletteResults.addEventListener('mousemove', (e) => {
  const item = e.target.closest('.palette-item');
  if (!item) return;
  const index = parseInt(item.dataset.index, 10);
  if (Number.isFinite(index) && index !== selectedIndex) {
    selectedIndex = index;
    renderPalette();
  }
});

paletteBackdrop.addEventListener('click', close);

// main -> renderer: show/hide palette
window.electronAPI.onPaletteShow(() => show());
window.electronAPI.onPaletteHide(() => {
  if (!isOpen) return;
  isOpen = false;
  palette.classList.add('hidden');
});
