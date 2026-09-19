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
```

## Releases

Tags of the form `v*.*.*` trigger `.github/workflows/release.yml` to
build and publish an AppImage.
