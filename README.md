# zebBrowser

Minimal browser for Linux. Built with Tauri, React and Rust. Starts blank, shows a centered search bar, hides the interface while reading.

## Install

One-line installer (handles dependencies for apt, dnf, pacman, zypper):

```sh
curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
```

Arch Linux:

```sh
makepkg -si
# or after AUR publish
yay -S zeb-browser
```

Manual download from [Releases](https://github.com/sahilcodexx/zebBrowser/releases).

## Usage

- Type a URL or search term and press Enter. Inputs without a scheme get `https://` prefixed, otherwise sent to DuckDuckGo.
- Hover near the top to reveal the address bar. Move away to hide.
- Shortcuts: `Ctrl+L` focus address bar, `Ctrl+R` / `F5` reload, `Alt+Left` / `Alt+Right` back/forward, `Esc` hide.

## Development

Dependencies for Arch:

```sh
sudo pacman -S webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg base-devel pnpm cargo
```

```sh
git clone https://github.com/sahilcodexx/zebBrowser.git
cd zebBrowser
pnpm install
pnpm tauri dev
```

Build:

```sh
pnpm tauri build
```

## Updates

The app checks for updates on launch. Releases are built by GitHub Actions when a tag is pushed:

```sh
# bump version in src-tauri/tauri.conf.json and package.json
git tag v0.1.1
git push origin v0.1.1
```

See `src-tauri/tauri.conf.json` for updater configuration and `.github/workflows/release.yml` for the workflow. Signing keys are in `~/.tauri/` and must be added as `TAURI_SIGNING_PRIVATE_KEY` in repository secrets.

## License

MIT
