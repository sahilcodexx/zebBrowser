# Mini Browser — Minimal Chromium Shell

> **Chromium engine kept, browser UI removed, your minimal UI added.**  
> One `BrowserWindow` + one `WebContentsView`. No tabs, no bookmarks, no history, no sync — just a floating search pill and 10–15 site shortcuts. Built for low-spec Linux recording workflow.

![Electron 31](https://img.shields.io/badge/Electron-31-47848F?logo=electron)
![Platform Linux](https://img.shields.io/badge/platform-Linux-lightgrey)
![License GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-green)

---

## Why this exists

You need to show 10–15 heavy sites (like `sahilcodex.vercel.app` with Next.js/Motion/GSAP) smoothly on a low-spec machine. Forking Helium/Chromium requires 100GB + C++ rebuilds for every UI tweak. This keeps the **Chromium engine** (so heavy sites stay smooth) and gives you a tiny HTML toolbar you can iterate with `npm`.

```
Your minimal browser
├── Minimal UI (HTML/CSS/JS)  → search bar, shortcuts, Zen hover
└── Chromium (Electron)       → Blink, V8, GPU, Web APIs, sandbox, multiprocess
```

## Features

- **Floating Zen pill** — thin `36px` pill, hidden by default, appears on hover near top (14px trigger), `Ctrl+L` to focus, auto-hides after navigation
- **Home grid** — default screen shows no site, just 15 shortcuts and `Search or enter URL` (edit in one place)
- **Smart address bar** — `example.com` → `https://example.com`, `localhost:3000` → `http://localhost:3000`, plain text → Google search
- **Shortcuts** — all in `main.js`, page-isolated:
  - `Ctrl+L` / `Cmd+L` focus bar (over any site)
  - `Enter` navigate/search, `Esc` close bar
  - `Ctrl+R` / `F5` reload, `Alt+←`/`Alt+→` back/forward, `⌂` home, `Ctrl+Shift+I` DevTools
- **Single view reused** — no tabs, no preloading, low RAM (one `WebContentsView`)

## Install

### Option 1 — AppImage (recommended, any distro)

Download the latest `*.AppImage` from [Releases](https://github.com/sahilcodexx/zebBrowser/releases/latest):

```bash
# example for v0.1.0 — check Releases for exact name
wget https://github.com/sahilcodexx/zebBrowser/releases/download/v0.1.0/Mini_Browser-0.1.0.AppImage
chmod +x Mini_Browser-0.1.0.AppImage
./Mini_Browser-0.1.0.AppImage
# or move to PATH
sudo mv Mini_Browser-0.1.0.AppImage /usr/local/bin/mini-browser
```

The AppImage is built by `.github/workflows/release.yml` on every tag push (`v*` or `zeb-v*`) via `electron-builder`.

### Option 2 — curl (one-liner, auto-detects distro/arch)

```bash
curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
# specific version
VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
```

What it does:

- Detects `x86_64`/`aarch64` and `apt`/`dnf`/`pacman`/`zypper` for `libfuse2` (AppImage dep)
- Resolves latest `AppImage` asset via GitHub API (handles spaced name `Mini Browser-*.AppImage`)
- Downloads to `~/.local/bin/mini-browser.AppImage`, creates wrapper `~/.local/bin/mini-browser` (Wayland quirks handled), desktop entry `~/.local/share/applications/mini-browser.desktop`, icon from `build/icon.png`
- Falls back to building from source (`pnpm install && pnpm run build`) if no release yet
- Ensures `~/.local/bin` is in `PATH`

Run: `mini-browser` or `mini-browser https://example.com`

### Option 3 — makepkg / AUR (Arch, CachyOS, Manjaro)

Build natively from source — no AppImage wrapping, Wayland-friendly:

```bash
git clone https://github.com/sahilcodexx/zebBrowser.git
cd zebBrowser
makepkg -si
# or clean chroot
# pkgctl build  # or aur helpers: yay -S mini-browser
```

`PKGBUILD` does:

- `pnpm install --frozen-lockfile` + `electron-builder --linux --dir`
- Installs to `/opt/mini-browser`, wrapper `/usr/bin/mini-browser` + `/usr/bin/mini`, desktop `mini-browser.desktop`, icons `512x512` etc.
- Depends: `gtk3 nss alsa-lib libxss libxtst libxcb libdrm at-spi2-core cairo pango`

To update `pkgver`, edit `PKGBUILD:4` or auto-bump: `pkgver=$(jq -r .version package.json)`.

## Quick Start (Development)

```bash
# from repo root (where package.json lives)
pnpm install
pnpm start
```

* Hover top edge → pill slides down
* Click a site card or type URL/search → loads full-window, pill hides
* `Ctrl+L` on any site → pill reappears over page, type next site, `Enter`

After changes, no C++ rebuild — just `pnpm start` again.

## Customize Your 10–15 Sites

Edit `main.js:5` — no UI changes needed:

```js
const SITES = [
  { label: 'SahilCodex', url: 'https://sahilcodex.vercel.app' },
  { label: 'GitHub',     url: 'https://github.com/sahilcodexx' },
  { label: 'Vercel',     url: 'https://vercel.com' },
  // ...
];
const SEARCH_ENGINE = 'https://www.google.com/search?q=';
```

## Shortcuts

| Key | Action |
|---|---|
| `Ctrl+L` | Focus floating search bar |
| `Enter` | Go to URL or search |
| `Esc` | Close bar (stay on site) |
| `Ctrl+R` / `F5` | Reload |
| `Alt+←` / `Alt+→` | Back / Forward (or home if no history) |
| `⌂` | Home grid |
| `Ctrl+Shift+I` | DevTools (detach) |

## Project Structure

```
.
├── main.js            # window, view, navigation, shortcuts (edit SITES here)
├── preload.js         # isolated bridge (contextBridge)
├── package.json       # start/build, electron-builder
├── pnpm-workspace.yaml
├── build/
│   ├── icon.png       # 818x834 minilogo → window + AppImage icon
│   └── icons/512x512.png
├── PKGBUILD           # makepkg -si
├── install.sh         # curl installer
├── mini-browser.desktop
└── ui/
    ├── index.html     # floating pill + home grid
    ├── style.css      # Zen pill + backdrop-blur
    └── renderer.js    # toolbar only
```

## Build

```bash
pnpm run build   # → dist/*.AppImage & dist/*.deb via electron-builder
# output: dist/Mini Browser-0.1.0.AppImage  dist/mini-browser_0.1.0_amd64.deb
```

Tag to release:

```bash
git tag v0.1.0 && git push origin v0.1.0
# GitHub Actions builds AppImage/deb and creates Release automatically
```

## Troubleshooting

**`Error: Electron failed to install correctly, please delete node_modules/electron`**

pnpm blocks build scripts by default. Ensure `pnpm-workspace.yaml` has:

```yaml
allowBuilds:
  electron: true
```

Then:

```bash
rm -rf node_modules/.pnpm/electron* node_modules/electron
pnpm install  # should show "postinstall$ node install.js Done" and download 101MB to ~/.cache/electron/
# if dist still only has locales, manually:
unzip -q ~/.cache/electron/*/electron-*.zip -d node_modules/electron/dist
echo -n electron > node_modules/electron/path.txt
```

`npm install` works without this but is slower. Warnings about slow tarball speed are harmless.

**GPU errors on Hyprland/Wayland**

```
GPU process launch failed: error_code=1002
zygote_communication_linux ... GetTerminationStatus
```

Usually harmless; app still opens (ignore `Fontconfig warning` spam). If white screen:

```bash
pnpm exec electron . --disable-gpu
# or
pnpm exec electron . --enable-features=UseOzonePlatform --ozone-platform=wayland
# installed AppImage wrapper respects: ELECTRON_DISABLE_GPU=1 mini-browser
```

**Benchmark for low specs**

After `pnpm start`, compare `sahilcodex.vercel.app`:

```bash
# in another terminal
ps -o pid,rss,comm -p $(pgrep -f mini-browser)
```

- If Electron is smooth → stay here.
- If RAM too high → keep same `ui/` and swap backend to CEF (same IPC contract, see `agent.md`).

## Credits

- Original `helium-linux` snapshot archived at `archive/pre-helium-fork` (`6b5b0b5f`) — to restore: `git checkout archive/pre-helium-fork -- docker patches scripts`
- Electron / Chromium
- Logo: `minilogo.png` (818×834) → `build/icon.png`

## License

GPL-3.0 — see `LICENSE` (if present).
