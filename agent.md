# ZebBrowser (`zeb`) — Complete Project Context & Architecture Guide

> **Zeb** is a minimalist, ultra-fast spotlight browser designed for distraction-free navigation. It combines the speed and ergonomics of a launcher (like Raycast/Spotlight) with a full-window native webview engine.

---

## 📑 Table of Contents
1. [Project Overview & Philosophy](#1-project-overview--philosophy)
2. [Tech Stack & Architecture](#2-tech-stack--architecture)
3. [Key Problems Solved & Technical Evolution](#3-key-problems-solved--technical-evolution)
4. [File & Directory Structure](#4-file--directory-structure)
5. [Core Components Deep-Dive](#5-core-components-deep-dive)
6. [Shortcuts & Navigation Engine](#6-shortcuts--navigation-engine)
7. [Installation & Distribution Pipeline](#7-installation--distribution-pipeline)
8. [Release & Versioning Workflow](#8-release--versioning-workflow)

---

## 1. Project Overview & Philosophy

### Core Concept
Traditional web browsers are cluttered with tabs, complex menus, extensions sidebars, and heavyweight chrome. **Zeb** reimagines web navigation:
1. **Launch Directly to Search**: A clean, centered spotlight input bar appears instantly upon opening.
2. **Instant Full-Window Browsing**: Typing a URL (or query) navigates directly to the target website taking up **100% of the window** at native display resolution.
3. **Frictionless Return**: Jump back to the spotlight search bar instantly using **`Ctrl+L`**, **`Esc`**, or by clicking the floating **`⌘`** Home button.
4. **Minimalist Aesthetic**: Pure white, Apple/Raycast-inspired design with soft shadows and crisp typography.

---

## 2. Tech Stack & Architecture

| Layer | Technology | Purpose |
|---|---|---|
| **Frontend UI** | React 19 + TypeScript | Spotlight search UI, keyboard handling, update dialogs |
| **Build Tool** | Vite 7 (`@vitejs/plugin-react`) | Rapid bundling and client compilation |
| **Desktop Framework** | Tauri v2 (`tauri 2.x`) | Native windowing, IPC, system integrations |
| **Backend / Native** | Rust 2021 | Window lifecycle, WebKitGTK injection, GPU flags |
| **Web Engine** | WebKitGTK 4.1 (Linux) | Rendering web content with hardware acceleration |
| **Updater** | Tauri Plugin Updater | Cryptographically signed OTA updates (minisign) |
| **Packaging** | AppImage, Deb, PKGBUILD | Multi-distro Linux distribution |
| **CI/CD** | GitHub Actions (`release.yml`) | Multi-target automated build & release pipeline |

---

## 3. Key Problems Solved & Technical Evolution

### Problem 1: `<iframe>` Embedding Restrictions (`X-Frame-Options`)
* **Issue**: Initial versions attempted to embed websites inside a React `<iframe>`. Major websites (GitHub, Google, Twitter, Vercel apps, banking portals) sent `X-Frame-Options: DENY` or `SAMEORIGIN`, resulting in refused connections.
* **Solution**: Migrated from iframe-based rendering to native top-level webview navigation where the webview navigates directly to the destination URL.

---

### Problem 2: GTK Multi-Webview Container Splitting ("Half-Screen Layout")
* **Issue**: Attempting to embed a child webview using `window.add_child` inside an existing GTK window container caused GTK3 to allocate 50% width/height to the parent React view and 50% to the child webview, splitting the window in half and distorting page layouts.
* **Solution**: Switched to **Single Full-Window Webview Architecture**. The window starts on the local frontend (`index.html`) and navigates directly to external URLs at 100% full-screen width & height without any split containers.

---

### Problem 3: WebKitGTK DMA-BUF Renderer Crashes on Linux Wayland
* **Issue**: On modern Linux Wayland compositors (Hyprland, Sway, GNOME) with Intel/Mesa or NVIDIA graphics, WebKitGTK crashed immediately on launch with:
  ```text
  Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
  ```
* **Solution**: Implemented `disable_webkit_dmabuf()` in Rust:
  ```rust
  #[cfg(target_os = "linux")]
  fn disable_webkit_dmabuf() {
      #[allow(unused_unsafe)]
      unsafe {
          if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
              std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
          }
      }
  }
  ```
  Called at the very beginning of `pub fn run()` prior to GTK initialization.

---

### Problem 4: Host Wayland Client Library Preload for AppImage
* **Issue**: AppImages bundling their own `libwayland-client.so.0` caused symbol conflicts with newer host Wayland compositors.
* **Solution**: Generated an intelligent wrapper in `install.sh` that detects the host's native `libwayland-client.so.0` and preloads it via `LD_PRELOAD`:
  ```bash
  for lib in /usr/lib/libwayland-client.so.0 /usr/lib/x86_64-linux-gnu/libwayland-client.so.0 /usr/lib64/libwayland-client.so.0; do
    if [ -f "$lib" ]; then
      export LD_PRELOAD="${LD_PRELOAD:+$LD_PRELOAD:}$lib"
      break
    fi
  done
  exec "$HOME/.local/bin/zeb.AppImage" "$@"
  ```

---

### Problem 5: Keyboard Shortcuts on External Websites & Wayland
* **Issue**: On Wayland, OS-level global shortcut hooks are blocked by compositors for security. Furthermore, when navigating to external websites (e.g. GitHub), React is unloaded and the external webpage captures all keyboard events.
* **Solution**: Injected a persistent **`initialization_script`** into the webview via `WebviewWindowBuilder`:
  - Executes at document-start on **EVERY** website before any page scripts run.
  - Catches `Ctrl+L`, `Esc`, `Ctrl+R`, `F5`, `Alt+Left`, `Alt+Right` in capture phase (`useCapture: true`).
  - Injects a sleek, floating **`⌘`** Home button on all external sites.

---

### Problem 6: Modern Desktop User-Agent
* **Issue**: Default WebKitGTK user agent caused Next.js / Vercel and GitHub to detect an outdated browser, serving fallback or broken mobile stylesheets.
* **Solution**: Configured standard Chrome 131 Desktop User-Agent:
  ```text
  Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36
  ```

### Problem 7: Heavy Sites (e.g. Vercel Portfolios) Freeze The Webview
* **Issue**: Opening a heavy site — many React components, several third-party analytics / ad scripts, lots of DOM — made the whole webview peg CPU and become unresponsive. The root cause was the side effect of Problem 3's DMA-BUF workaround: with `WEBKIT_DISABLE_DMABUF_RENDERER=1` forced, WebKitGTK falls back to a software render pipeline, which is dramatically slower for any non-trivial page. The secondary cause was the cost of running analytics / ad scripts in parallel to the React tree (Vercel Analytics, Sentry, Hotjar, etc.).
* **Solution**:
  1. **Make hardware acceleration opt-in**, not forced. `apply_webkit_env()` in `src-tauri/src/lib.rs` now reads `ZEB_HARDWARE_ACCEL=1` and only forces `WEBKIT_DISABLE_DMABUF_RENDERER=1` when the user has *not* opted in. The safe software path stays the default so the original EGL crash fix is preserved.
  2. **Lightweight JS content blocker** injected at document-start. A small IIFE overrides `fetch`, `XMLHttpRequest`, `sendBeacon` and a `MutationObserver` to short-circuit requests to a curated list of tracker / ad domains (Google Analytics, Vercel Analytics, Sentry, Hotjar, FullStory, LogRocket, Mixpanel, Segment, Amplitude, Heap, Datadog, Intercom, Zendesk, Tawk.to, Plausible, Umami, Cloudflare Insights, Yandex, Quantcast, Comscore, ad exchanges, etc.). Skips `localhost` and `tauri://` pages.
  3. **Opt-out flag**: `ZEB_DISABLE_CONTENT_BLOCKER=1` sets `window.__ZEB_NO_BLOCK__ = true` and the blocker bails out — useful when a site legitimately depends on one of the blocked SDKs.
  4. **Disk cache hint**: `WEBKIT_CACHE_DIR` is set to `$XDG_CACHE_HOME/zeb` so repeat visits to the heavy portfolio skip the network entirely.

### Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `ZEB_HARDWARE_ACCEL=1` | off | Re-enable DMA-BUF / GPU rendering on systems that don't suffer the Wayland EGL crash. Largest single perf win. |
| `ZEB_DISABLE_CONTENT_BLOCKER=1` | off | Disable the JS tracker / ad blocker if a site breaks. |
| `ZEB_LITE=1` | off | Enable **Lite Mode** on every external page (see Problem 8). The "feels smooth" switch. |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | forced | Legacy env var. If `ZEB_HARDWARE_ACCEL=1` is set, this is left untouched (so the user can still force the old behavior). |

### Problem 8: Lite Mode — making heavy sites *feel* smooth
* **Issue**: Even with the content blocker off the critical path, a React + Framer-Motion / GSAP portfolio still janks while scrolling: every animation paints every frame on the software render path, every <img> decodes synchronously on first paint, and any `WebGL` context adds multi-hundred-ms of GPU init. The site is *correct*, but the experience is "laggy".
* **Solution**: A new `LITE_MODE` constant is appended to the inject script (gated on `__ZEB_LITE__`, set from `ZEB_LITE=1`). On every non-local page it:
  1. Inserts a `!important` stylesheet that clamps every `animation-*` / `transition-*` duration to 0.001ms and forces `scroll-behavior: auto`. Single biggest jank-killer.
  2. Walks every <img> (now and via MutationObserver) setting `loading="lazy"` and `decoding="async"` so the browser only decodes what's actually on-screen.
  3. Wraps `HTMLCanvasElement.prototype.getContext` to return `null` for `webgl` / `webgl2` / `experimental-webgl`. Most portfolios don't need them.
  4. Tags `<html>` with `zeb-lite` so the user can target it from DevTools or their own CSS.
* **UX surface**: a small `?` chip in the bottom-left of the spotlight screen opens a modal (`get_perf_settings` Tauri command) that reads the current env-var state and shows what each toggle does, plus the env var name to set. No runtime mutation — env vars are process-level — so the modal is honest about restart-required.

---

## 4. File & Directory Structure

```
mini/
├── .github/
│   └── workflows/
│       └── release.yml          # GitHub Actions multi-target build & release workflow
├── src/
│   ├── App.tsx                  # React Spotlight start screen & updater modal
│   ├── App.css                  # Pure white minimalist stylesheet
│   ├── main.tsx                 # React entry point
│   └── index.css                # Global CSS resets
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs               # Rust backend, IPC commands, injected runtime script
│   │   └── main.rs              # Binary entry point invoking lib.rs
│   ├── capabilities/
│   │   └── default.json         # Tauri v2 security capabilities & permissions
│   ├── icons/                   # App icons (32x32, 128x128, 512x512, icon.png)
│   ├── Cargo.toml               # Rust dependencies and package configuration
│   └── tauri.conf.json          # Tauri configuration (updater pubkey, window options)
├── index.html                   # HTML template
├── install.sh                   # Universal Linux installer (Deb, RPM, AppImage, Arch)
├── package.json                 # Node dependencies and project metadata
├── PKGBUILD                     # Arch Linux packaging script
├── tsconfig.json                # TypeScript compiler configuration
├── vite.config.ts               # Vite configuration with React & Tauri port
└── zeb.desktop                  # Linux XDG Desktop Entry
```

---

## 5. Core Components Deep-Dive

### `src-tauri/src/lib.rs`
The backend hub of the application:
1. **`INJECT_SCRIPT`**: JavaScript injected into every webpage at document creation:
   - Captures `Ctrl+L` / `Esc` -> Navigates to `window.__ZEB_DEV__ ? 'http://localhost:1420' : 'tauri://localhost'`.
   - Captures `Ctrl+R` / `F5` -> `window.location.reload()`.
   - Captures `Alt+Left` / `Alt+Right` -> `window.history.back()` / `forward()`.
   - Injects floating `⌘` Home button on non-localhost domains.
2. **IPC Commands**:
   - `navigate_browser(url)`: Navigates main window to parsed URL.
   - `go_home()`: Navigates back to the local spotlight start screen.
   - `browser_reload()`: Reloads current page.
   - `browser_go_back()` / `browser_go_forward()`: History navigation.
3. **Window Setup (`WebviewWindowBuilder`)**:
   - Frameless window (`decorations: false`).
   - Sizing: `1200x800` (min `800x500`).
   - Shadow & resize enabled.
   - User-Agent & DevTools enabled.

### `src/App.tsx`
The Spotlight frontend:
1. **Search Bar**: Centered auto-focused input with instant query resolution via `makeUri()`:
   - URLs (`github.com`, `http://...`) are normalized to `https://...`.
   - Plain text queries redirect to DuckDuckGo search.
2. **Auto-Updater**: Integrates `@tauri-apps/plugin-updater` to poll for new releases and prompt for 1-click update and relaunch.
3. **Keyboard Controls**: `Ctrl+L` immediately focuses and selects the search input.

### `src/App.css`
Minimalist white design tokens:
- Background: `#f8fafc`.
- Card / Input: `#ffffff` with `#e2e8f0` subtle border and soft multi-layer shadow `rgba(0,0,0,0.07)`.
- Accent / Focus: `#3b82f6` with 3px focus ring.
- Typography: System font stack (`-apple-system, BlinkMacSystemFont, 'Inter', sans-serif`).

---

## 6. Shortcuts & Navigation Engine

| Shortcut | Action | Behavior |
|---|---|---|
| **`Enter`** | Navigate / Search | Normalizes URL/query and navigates full-screen |
| **`Ctrl + L`** | Spotlight Home | Returns immediately to the search bar |
| **`Escape`** | Spotlight Home | Returns to search (when not typing in form inputs) |
| **`Ctrl + R` / `F5`** | Reload | Reloads the active webpage |
| **`Alt + ←`** | Back | Navigates back in history |
| **`Alt + →`** | Forward | Navigates forward in history |
| **`⌘` Button** | Click Home | Bottom-right floating button to return to search |

---

## 7. Installation & Distribution Pipeline

### Universal Installer (`install.sh`)
Supports all major Linux distributions:
```bash
curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
```
* **Ubuntu / Debian / Mint / Pop!_OS**: Installs `.deb` package or falls back to AppImage.
* **Fedora / RHEL / Rocky**: Installs `.rpm` package or falls back to AppImage.
* **Arch Linux / CachyOS / Manjaro**: Builds natively via `PKGBUILD` for maximum Wayland performance.
* **Any other distro**: Installs AppImage with Wayland EGL wrapper to `~/.local/bin/zeb`.

### Arch Linux PKGBUILD
Builds directly from source using system libraries:
```bash
makepkg -si
```

---

## 8. Release & Versioning Workflow

To publish a new release:
1. **Bump Version** in all 4 configuration files:
   - `package.json` (`"version": "X.Y.Z"`)
   - `src-tauri/tauri.conf.json` (`"version": "X.Y.Z"`)
   - `src-tauri/Cargo.toml` (`version = "X.Y.Z"`)
   - `PKGBUILD` (`pkgver=X.Y.Z`)
2. **Commit & Tag**:
   ```bash
   git add -A
   git commit -m "feat: release vX.Y.Z"
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin main
   git push origin vX.Y.Z
   ```
3. **Automated Pipeline**:
   - GitHub Actions workflow (`.github/workflows/release.yml`) builds AppImage, `.deb`, and updater signatures.
   - Automatically publishes the release and updates `latest.json`.
