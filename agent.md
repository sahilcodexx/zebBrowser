# agent.md — Mini Browser (Electron) — Agent Context

> Read this before making any changes. Single source of truth for architecture, IPC contract, and Zen UI behavior.

## 1. Project Overview

Mini Browser is a **minimal Chromium shell** for personal use on low-spec Linux (CachyOS/Hyprland). Goal is not to rebuild Chromium, but to provide a tiny custom UI around Electron's bundled Chromium.

**Diagram intent (from user):**
```
helium/Chromium engine KEEP (Blink, V8, GPU, networking, Web APIs, sandbox, multiprocess)
browser UI REMOVE (tabs, bookmarks, history, sync, onboarding, sidebar, menus)
your minimal UI ADD (single search bar — no link grid, no toolbar)
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
│   ├── index.html       # home: centered search input
│   ├── style.css        # home pill styles
│   ├── renderer.js      # home show/hide, navigation, Ctrl+D → request palette
│   ├── palette.html     # command palette UI (loaded into its own WebContentsView)
│   ├── palette.css      # command palette styles (backdrop, card, items, footer)
│   └── palette.js       # palette show/close/filter, executes actions via main
adblocker/
└── lists/
    └── default.txt      # bundled starter list (~300 ad/tracking domains)
└── agent.md / README.md
```

No `mini-browser/` subfolder anymore — contents were flattened to root (see image proof `mini/ui` at root). `node_modules/.pnpm/electron@*` holds the binary.

## 4. Key Files

### `main.js:1` — Main Process (navigation lives here)

- `SEARCH_ENGINE:4` — `https://www.google.com/search?q=`
- `viewVisible:5` — controls whether the **site** `WebContentsView` is shown. View is full-window on a site, `0x0` on home.
- `isUrl():10` / `toUrl():18` — URL vs search detection (http, localhost, domain vs query).
- `updateViewBounds():27` — if `!viewVisible` → `0x0`, else full `getContentBounds()`. Called on `resize`.
- `showView():37` / `hideView():43` — toggle `viewVisible`, send `view-visibility` to home, `hideView` also `loadURL('about:blank')`.
- `showPalette():54` / `hidePalette():61` — palette view bounds: `0x0` when closed, full window when open. `hidePalette` restores focus to the view (if a site is showing) or the home page.
- `createWindow():69` — `BrowserWindow` (1280x800, `hiddenInset`, `autoHideMenuBar`) loads `ui/index.html`; then `view` (site) and `paletteView` (palette) are added as children of `win.contentView` in that order, so the palette renders on top.
- `did-navigate:122` auto-`showView()` when URL != `about:blank`, sync URL via `url-changed`.
- `handleShortcut():154` — `Ctrl/Cmd+L` → `win.focus()` + `focus-address-bar`, `Ctrl/Cmd+K` → `win.focus()` + `showPalette()`, `Ctrl+R/F5` reload, `Ctrl+Shift+I` DevTools, `Alt+←/→` back/forward, `Esc` stop. Only the **view** listens via `before-input-event` so the win/palette can own `Esc`/`↑`/`↓`/`Enter`/`Ctrl+K`.
- IPC handlers — `navigate` (calls `showView`+`loadURL`), `go-home`→`hideView`, `go-back` (or `hideView` if no history), `go-forward`/`reload`/`stop`/`focus-view`; `show-palette-request` from home; `palette-close` and `palette-action` from the palette view (`{type: 'go-home'|'go-back'|'go-forward'|'reload'|'navigate', query?}`).

### `preload.js:1` — Bridge (isolated)

Exposes `window.electronAPI` via `contextBridge`. Only `invoke`/`send`/`on` wrappers. No direct `ipcRenderer` exposure. Surface (used by both home and palette webContents):
- Navigation: `navigate`, `goBack`, `goForward`, `reload`, `stop`, `goHome`, `focusView`.
- Home events: `onFocusAddressBar`, `onUrlChanged`, `onViewVisibility`.
- Palette plumbing: `requestShowPalette` (home → main), `onPaletteShow` (main → palette), `paletteAction` (palette → main), `closePalette` (palette → main).

### `ui/index.html` and `ui/style.css` — home

- `#home` — full-bleed centered container, hidden (`.hidden`) while a site is showing.
- `#search-form` — single inline form with a search SVG icon and the `#search` text input. 48px white pill with `border-radius:999px`, soft shadow; `:focus` swaps to a darker border and a stronger shadow.

### `ui/renderer.js` — home

- `viewHasContent:10` — mirrors `viewVisible` from main.
- `showHome(show):12` — toggles `home.hidden`, sets `viewHasContent`, and on show schedules `search.focus()` + `select()` (10ms delay for Wayland focus).
- `navigate(value):21` → `electronAPI.navigate`, `showHome(false)`, `focusView` (80ms).
- `goHome():29` → clear search, `showHome(true)`, `goHome` IPC.
- `onFocusAddressBar:41` (from main on Ctrl/Cmd+L) → if a site is showing, `goHome()`; else just refocus + select.
- `Ctrl/Cmd+K` on home → `electronAPI.requestShowPalette()`.
- `onViewVisibility:60` syncs from main; `onUrlChanged:50` mirrors current URL into `viewHasContent`.

### `ui/palette.html`, `ui/palette.css`, `ui/palette.js` — command palette

- Loaded into its own `WebContentsView` (see main.js). When closed, the view is at `0x0`; when open, full-window on top of the site view.
- `STATIC_COMMANDS` (Go Home/Back/Forward/Reload) + dynamic `navigate` action when input is non-empty. `getCommands` filters by substring on label+keywords. `renderPalette` groups by `section` and highlights the selected item.
- `paletteInput` keydown handles `Esc`/`↑`/`↓`/`Enter`/`Ctrl+K`; `paletteResults` click+mousemove; `paletteBackdrop` click sends `closePalette`.
- `onPaletteShow` (from main) calls `show()`; `paletteAction({type, query?})` triggers the action in main, which closes the palette automatically.

### `pnpm-workspace.yaml:1`

`allowBuilds: electron: true` — **required** for pnpm. Without it `pnpm install` shows `[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: electron` and `install.js` never downloads binary → `Error: Electron failed to install correctly` (`node_modules/electron/index.js:17`). Fix was `pnpm approve-builds` selecting electron. If pnpm version changes, edit this file.

## 5. Architecture

```
BrowserWindow (win) — loads file:// ui/index.html (home)
└── contentView
    ├── webContents (root, home: centered search input)
    ├── WebContentsView (view)         — loads https:// sites, full window when active
    └── WebContentsView (paletteView)  — loads file:// ui/palette.html, 0x0 when closed
```
The palette view is added **after** the site view, so it renders on top of the site when its bounds are full-window. Closed = `0x0` (invisible). Open = full content bounds (visible above the site).

Separation: `ui/renderer.js` and `ui/palette.js` never touch `webContents` directly, only via IPC to `main.js`. Security: `contextIsolation:true, sandbox:true, nodeIntegration:false` in all three.

## 6. IPC Contract

| Renderer → Main | Main → Renderer |
|---|---|
| `send navigate` (string) | `view-visibility` (bool) → home |
| `send go-back/go-forward/reload/stop/go-home/focus-view` | `url-changed` (url) → home |
| `send show-palette-request` (home) | `focus-address-bar` → home |
| `send palette-close` (palette) | `palette-show` → palette |
| `send palette-action` (palette, `{type, query?}`) | |

## 7. Shortcuts

| Shortcut | Location | Action |
|---|---|---|
| `Ctrl/Cmd+L` | `main.js` (view) | Bring window forward + signal home; home goes to search bar |
| `Ctrl/Cmd+K` | `main.js` (view) / `renderer.js` (home) | Open command palette |
| Palette `↑` / `↓` | `palette.js` | Move selection |
| Palette `Enter` | `palette.js` | Send `palette-action` to main → execute + close |
| Palette `Esc` / `Ctrl+K` | `palette.js` | Send `palette-close` to main |
| `Enter` in home search | `renderer.js` | Submit → `navigate()` |
| `Ctrl+R` / `F5` | `main.js` | Reload view |
| `Alt+←` | `main.js` | Back (or home if no history) |
| `Alt+→` | `main.js` | Forward |
| `Esc` (view) | `main.js` | Stop loading |
| `Esc` (home) | `renderer.js` | If a site is showing, go home |
| `Ctrl+Shift+I` | `main.js` | Toggle DevTools (detach) |

### Command palette (Ctrl+D)

- Static commands: `Go Home` (⌂), `Go Back` (←), `Go Forward` (→), `Reload Page` (↻).
- Privacy section: `Ad Blocker: On · N blocked` (toggle) and `Update Ad Blocker List`.
- Dynamic action: when the user has typed something, a top "Action" item appears — `Go to "<query>"` if the input looks like a URL (`http(s)://`, `localhost:port`, or `domain.tld`), otherwise `Search for "<query>"`. Selecting it sends `palette-action` to main, which routes through the same `toUrl` / view-load path.
- Backdrop click, `Esc`, or `Ctrl+D` again sends `palette-close`; main hides the palette view and restores focus to the view (if a site is open) or the home page.

### Ad blocker (uBlock-style, network-level)

- Runs in `main.js` via `session.defaultSession.webRequest.onBeforeRequest` with a `http(s)://*/*` URL filter — every request is matched against the in-memory `adDomains` Set by walking parent domains (e.g. `cdn.ads.example.com` → `ads.example.com` → `example.com`). Default ON, persisted to `userData/config.json`.
- Domain list: `adblocker/lists/default.txt` (~300 entries, curated). Parsed by `loadAdListFromText` which accepts both bare domains and hosts format. On startup, main prefers `userData/adblocker-list.txt` (saved by an Update) and falls back to the bundled list.
- Counter: each cancelled request increments `blockedCount`; main throttles `adblocker-update` broadcasts to ≤1 per 400ms and pushes `{ enabled, blockedCount }` to both the home webContents and the palette webContents.
- Palette toggle: `Ad Blocker: On · N blocked` / `Ad Blocker: Off` in the Privacy section. Sends `palette-action { type: 'adblocker-toggle' }`; main flips the flag, saves config, broadcasts.
- Palette update: `Update Ad Blocker List` fetches `https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts` (HTTPS, 15s timeout, one redirect), parses it, replaces the in-memory Set, resets the counter, and persists to `userData/adblocker-list.txt`. Result comes back through the same `adblocker-update` event.

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
# on launch: centered search input; type URL/search → Enter
# Ctrl+K (on any site or on home) → command palette; Esc / Ctrl+K / backdrop click to close
pnpm run build  # electron-builder → AppImage/deb
```

**Do not** run `pnpm start` from `mini-browser/` subfolder — project is at root now.

## 9. Configuration

Edit `main.js:4` `SEARCH_ENGINE` to change the default web search. No rebuild needed. The link grid was removed, so add sites back by reintroducing a `SITES` array and a `get-sites` IPC if you ever want shortcuts again.

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
