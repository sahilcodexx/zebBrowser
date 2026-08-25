# agent.md — Mini Browser (Electron) — Agent Context

> Read this before making any changes. Single source of truth for architecture, IPC contract, and Zen UI behavior.

## 1. Project Overview

Mini Browser is a **minimal Chromium shell** for personal use on low-spec Linux (CachyOS/Hyprland). Goal is not to rebuild Chromium, but to provide a tiny custom UI around Electron's bundled Chromium.

**Diagram intent (from user):**
```
helium/Chromium engine KEEP (Blink, V8, GPU, networking, Web APIs, sandbox, multiprocess)
browser UI REMOVE (tabs, bookmarks, history, sync, onboarding, sidebar, menus)
your minimal UI ADD (search bar + 10-15 shortcuts)
```
Achieved via **Electron**: `BrowserWindow (toolbar) + WebContentsView (website)`.

**Current repo state:** Original `helium-linux` files (`docker/`, `patches/`, `scripts/`, `package/`, etc.) have been **replaced** by the mini-browser at repo root (`main.js`, `preload.js`, `ui/`). The snapshot before the overwrite is tagged `archive/pre-helium-fork` (`6b5b0b5f`). To restore helium-linux: `git checkout archive/pre-helium-fork -- docker patches scripts package flags.linux.gn`.

## 2. Tech Stack

- **Runtime:** Electron 31.7.7 (Chromium + Node), Node 24 (dev), pnpm 11
- **Frontend:** Vanilla HTML/CSS/JS (no framework) in `ui/`
- **Build:** electron-builder 24.13.3 (AppImage/deb)
- **OS:** CachyOS (Arch) Wayland/Hyprland — test via `pnpm start`

## 3. Directory Structure

```
mini/ (repo root, was helium-linux)
├── main.js              # main process: window, view, shortcuts, navigation
├── preload.js           # isolated bridge (contextBridge)
├── package.json         # scripts: start/dev/build, electron-builder config
├── pnpm-workspace.yaml  # allowBuilds: electron: true  (required for pnpm)
├── pnpm-lock.yaml
├── ui/
│   ├── index.html       # toolbar (floating pill) + home grid
│   ├── style.css        # Zen hover pill + home grid
│   └── renderer.js      # toolbar-only logic, no navigation
└── agent.md / README.md
```

No `mini-browser/` subfolder anymore — contents were flattened to root (see image proof `mini/ui` at root). `node_modules/.pnpm/electron@*` holds the binary.

## 4. Key Files

### `main.js:1` — Main Process (navigation lives here)

- `SITES:5` — array of 15 `{label, url}`. Edit here to change home shortcuts, no UI touch.
- `SEARCH_ENGINE:23` — `https://www.google.com/search?q=`
- `viewVisible:25` — controls whether `WebContentsView` is shown. `TOOLBAR_HEIGHT=0` — view is full-window, toolbar floats over it.
- `isUrl():30` / `toUrl():38` — URL vs search detection (http, localhost, domain vs query).
- `updateViewBounds():47` — if `!viewVisible` → `0x0`, else full `getContentBounds()`. Called on `resize`.
- `showView():57` / `hideView():62` — toggle `viewVisible`, send `view-visibility` to renderer, `hideView` also `loadURL('about:blank')`.
- `createWindow():70` — `BrowserWindow` (1280x800, `hiddenInset`, `autoHideMenuBar`) loads `ui/index.html`; `WebContentsView` added as child, initially `hideView()`.
- IPC + `did-navigate:115` auto-`showView()` when URL != `about:blank`, sync URL/loading to toolbar.
- `handleShortcut():137` — `Ctrl/Cmd+L` → `win.focus()` + `focus-address-bar`, `Ctrl+R/F5` reload, `Ctrl+Shift+I` DevTools, `Alt+←/→` back/forward, `Esc` stop. Both `win` and `view` listen via `before-input-event:181` with `event.preventDefault()`.
- IPC handlers `193` — `get-sites`, `get-current-url`, `navigate` (calls `showView`+`loadURL`), `go-home`→`hideView`, `go-back` (or `hideView` if no history), etc.

### `preload.js:1` — Bridge (isolated)

Exposes `window.electronAPI` via `contextBridge`. Only `invoke`/`send`/`on` wrappers. No direct `ipcRenderer` exposure. New APIs: `goHome`, `onViewVisibility`, `onShowToolbar`.

### `ui/index.html:1`

- `#hover-trigger:10` — 14px transparent zone at top, triggers pill.
- `#toolbar:12` — floating pill with `homeBtn`, `back`, `forward`, `reload`, `address-wrap` (🔍 + input + loading), `stop`. Fixed, centered.
- `#home:27` — centered grid, `h1`, `p`, `#sites`, `#hint`.

### `ui/style.css:1`

- `#hover-trigger:6` — fixed top 0 height 14px.
- `#toolbar:12` — fixed `top:10px left:50% transform: translateX(-50%)`, `width: min(640px,62%) height:36px`, `border-radius:999px`, `backdrop-filter: blur(12px)`, `opacity:0` + `translateY(-16px)` hidden, visible when `.visible` or `#hover-trigger:hover + #toolbar` or `:hover` or `:focus-within` → `opacity:1 translateY(0)`.
- Buttons `26x26`, `address-wrap 26px`, thin pill. `#home` is `flex:1` grid centered, `max-width:720px`, site grid `repeat(auto-fill, minmax(140px,1fr))`.

### `ui/renderer.js:1`

- `viewHasContent:14` — mirrors `viewVisible` from main.
- `showHome(show):23` — toggles `home.hidden` and toolbar `.visible`; home→ pinned visible, site→ auto-hide.
- `navigate(value):35` → `electronAPI.navigate`, `showHome(false)`, `focusView`.
- `goHome():43` → clear input, `showHome(true)`, `goHome()` IPC.
- Toolbar btn handlers, `address` Enter→navigate, Esc→blur+hide or close, Ctrl+L→select.
- Hover `85` keeps pill visible, `mouseleave` hides if not focused.
- `onFocusAddressBar:93` — **does not toggle home**, just `toolbar.visible` + `address.focus()` + `select()` (delay 10ms for Wayland).
- `onViewVisibility:116` syncs from main, `onUrlChanged`, `onLoadingChanged`, site grid build from `getSites()`.

### `pnpm-workspace.yaml:1`

`allowBuilds: electron: true` — **required** for pnpm. Without it `pnpm install` shows `[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: electron` and `install.js` never downloads binary → `Error: Electron failed to install correctly` (`node_modules/electron/index.js:17`). Fix was `pnpm approve-builds` selecting electron. If pnpm version changes, edit this file.

## 5. Architecture

```
BrowserWindow (win) — loads file:// ui/index.html
├── #hover-trigger + #toolbar (HTML, renderer.js)
└── contentView
    └── WebContentsView (view) — loads https:// sites, full window
```
Separation: `ui/renderer.js` never touches `webContents` directly, only via IPC to `main.js`. Security: `contextIsolation:true, sandbox:true, nodeIntegration:false` in both.

## 6. IPC Contract

| Renderer → Main | Main → Renderer |
|---|---|
| `invoke get-sites` | `view-visibility` (bool) |
| `invoke get-current-url` | `url-changed` (url) |
| `send navigate` (string) | `loading-changed` (bool) |
| `send go-back/go-forward/reload/stop/go-home/focus-view` | `focus-address-bar` |
| `send show-toolbar` | `show-toolbar` |

## 7. Shortcuts

| Shortcut | Location | Action |
|---|---|---|
| `Ctrl/Cmd+L` | `main.js:144` | Focus address bar (floating pill) |
| `Enter` in address | `renderer.js:43` | `toUrl()` → navigate or search |
| `Ctrl+R` / `F5` | `main.js:152` | Reload view |
| `Alt+←` | `main.js:163` | Back (or home if no history) |
| `Alt+→` | `main.js:168` | Forward |
| `Esc` (view) | `main.js:173` | Stop loading |
| `Esc` (address) | `renderer.js:50` | Blur+hide pill, keep site |
| `⌂` | `renderer.js:51` | Go home (`about:blank` + hide view) |
| `Ctrl+Shift+I` | `main.js:157` | Toggle DevTools (detach) |

## 8. Development Workflow

```bash
# install (pnpm required allowBuilds)
pnpm install

# if you see "Electron failed to install correctly":
# 1. ensure pnpm-workspace.yaml has electron: true
# 2. rm -rf node_modules/.pnpm/electron* node_modules/electron
# 3. pnpm install  # should show "postinstall$ node install.js Done" and download ~101MB zip to ~/.cache/electron/
# 4. if still only locales/zh-CN.pak in dist, manually: unzip ~/.cache/electron/*/electron-*.zip -d node_modules/electron/dist && echo -n electron > node_modules/electron/path.txt

pnpm start   # or npm start (if pnpm slow)
# hover top 14px → pill appears; Ctrl+L → focus; type URL/search → Enter
pnpm run build  # electron-builder → AppImage/deb
```

**Do not** run `pnpm start` from `mini-browser/` subfolder — project is at root now.

## 9. Configuration

Edit `main.js:5` `SITES` and `SEARCH_ENGINE:23`. No rebuild needed. Example: add `{label:'Linear', url:'https://linear.app'}`.

## 10. Known Issues & Gotchas

- **pnpm ignored builds:** Must allow electron. Symptom: `dist` only has `locales/zh-CN.pak`, missing `electron` binary + `path.txt`. See workflow above. `npm install` works without this but slower.
- **Manual electron extract:** Cached zip `~/.cache/electron/c94.../electron-v31.7.7-linux-x64.zip` (101MB) was valid but `install.js` didn't extract due to pnpm isolated store + `isInstalled()` early return. Manual `unzip -q cache.zip -d dist && echo -n electron > path.txt` fixed.
- **Wayland/Hyprland GPU:** `pnpm start` may log `GPU process launch failed: error_code=1002`, `zygote_communication_linux`, `command_buffer_proxy_impl` — usually harmless, app still opens. If white screen, launch with `--disable-gpu` or `--enable-features=UseOzonePlatform --ozone-platform=wayland`.
- **Fontconfig warnings:** `48-guessfamily.conf xsi:nil` spam — cosmetic, ignore (`grep -v Fontconfig`).
- **Node version:** Electron 31 tested with Node 24/26, but Node 22 LTS safest. `node -v` is 26.7.0 currently.
- **Git history:** Root now untracked deletions of helium-linux. `origin/main` still at `6b5b0b5f` (helium `feat(perf)...`). Local ahead after scaffold not pushed. `archive/pre-helium-fork` tag is backup.
- **ViewVisible vs home:** View is `0x0` on home, full on site. Don't set `TOOLBAR_HEIGHT` back to 48 — floating design needs `0`.

## 11. Future

- Benchmark `sahilcodex.vercel.app` in Electron vs Chrome (`ps -o pid,rss`). If RAM too high, keep same `ui/` and swap backend to CEF (same IPC).
- No need to delete `components/bookmarks` etc. — just not exposing UI is enough.

## 12. Commands for Agents

- Read `main.js`, `preload.js`, `ui/*` before edits.
- Keep navigation in `main.js`, UI in `renderer.js`.
- Keep `WebContentsView` (not deprecated `BrowserView`).
- Test with `timeout 6 pnpm start` (SIGTERM expected).
