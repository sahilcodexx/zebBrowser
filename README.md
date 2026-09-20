# Mini Browser

Zen-inspired minimal browser built with **Rust + GTK4 + WebKitGTK 6** on Linux.

## Highlights

- **Auto-hide topbar** – moves out of the way so pages fill the window;
  hover the top edge to bring it back (Zen-browser style).
- **Tab pills** – compact top-row tabs with pin support and hover-only close.
- **Traffic-light window controls** – proper 12 × 12 circles, not stretched.
- **New Tab** label never exposes the raw `data:` URI used to render the centered-search page.
- **URL bar always visible** when you need it; nice and small when you don't.

## Run from source

```bash
cargo run --release
```

## Install AppImage

Download the latest release, make it executable, and run it:

```bash
chmod +x Mini-Browser-v*-x86_64.AppImage
./Mini-Browser-v*-x86_64.AppImage
```

The AppImage is **type-1** — it extracts itself to a temp directory at launch,
so it does not need FUSE to be installed or mounted. Double-clicking from a
file manager works the same way as running from a terminal.

### Troubleshooting

- **Nothing happens when I double-click the AppImage.**
  Open a terminal in the same directory and run it directly:
  ```bash
  ./Mini-Browser-v*-x86_64.AppImage
  ```
  If launched without an attached terminal, the wrapper also writes a log to
  `~/.local/share/mini-browser/launch.log` (or `/tmp/mini-browser-logs/launch.log`
  if your home is read-only). Read that file to see what went wrong.

- **`error while loading shared libraries: libwebkitgtk-6.0.so.0`**.
  This should not happen with the bundled AppImage — all GTK/WebKit libraries
  are included. If you do see it, your download is probably truncated. Re-download
  and re-`chmod +x`.

- **Window opens but the page is blank / black.**
  Some Wayland compositors (especially Nvidia + GNOME) struggle with the GPU
  compositor path. The wrapper already sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
  If you still hit it, try forcing X11:
  ```bash
  GDK_BACKEND=x11 ./Mini-Browser-*.AppImage
  ```

- **Build fails locally with `webkitgtk-6.0 development files not found`.**
  You need WebKitGTK 6.0 from your distro. On Ubuntu 24.04 / Debian 13+:
  ```bash
  sudo apt install libwebkitgtk-6.0-dev libgtk-4-dev
  ```

## Keyboard

| Shortcut | Action |
|---|---|
| `Ctrl+L` | Focus address bar |
| `Ctrl+T` | New tab |
| `Ctrl+W` | Close tab |
| `Ctrl+R` / `F5` | Reload |
| `Alt+Left` / `Alt+Right` | Back / forward |
| `Escape` | Stop loading |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |
| `Ctrl+1..9` | Switch to tab N |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+P` | Pin / unpin current tab |

## Layout

```
src/
├── main.rs        app entry point
├── app.rs         Application wiring
├── window.rs      BrowserWindow (chrome, tabs, revealer/overlay, motion hover)
├── tab.rs         Tab state model + TabEvent callback
├── navigation.rs  URL vs search routing
├── config.rs      App name, home, new-tab HTML, error HTML
├── webview.rs     WebView wrapper
└── webkit_ffi.rs  Raw WebKitGTK 6 FFI declarations
packaging/
├── AppRun-wrapper.sh   AppImage AppRun: safe defaults + log fallback
├── mini-browser.desktop
└── mini-browser.png    818×834 source icon (resized to 256×256 in CI)
```

## Releases

Tags of the form `v*.*.*` trigger `.github/workflows/release.yml` to
build and publish an AppImage.