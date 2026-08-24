# Maintainer: sahilcodexx <sahil171759@gmail.com>
# Build from this repo (run `makepkg -si` in the project root).
# Uses system webkit2gtk — do not wrap the GitHub AppImage (blank window on Hyprland).
pkgname=zeb-browser
pkgver=0.1.6
pkgrel=1
pkgdesc="Minimal spotlight browser — centered pill, hover Zeb bar, Tauri"
arch=('x86_64' 'aarch64')
url="https://github.com/sahilcodexx/zebBrowser"
license=('MIT')
depends=('webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg' 'cairo' 'gtk3')
makedepends=('cargo' 'nodejs' 'npm' 'pkgconf')
source=()
sha256sums=()
options=('!debug')

build() {
  cd "$startdir"
  export CARGO_HOME="${srcdir}/cargo-home"
  unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS RUSTFLAGS
  if command -v pnpm >/dev/null 2>&1; then
    pnpm install --frozen-lockfile || pnpm install
    pnpm build
  else
    npx --yes pnpm install --frozen-lockfile || npx --yes pnpm install
    npx --yes pnpm build
  fi
  cargo build --release --manifest-path src-tauri/Cargo.toml
}

package() {
  cd "$startdir"
  install -Dm755 src-tauri/target/release/zeb "${pkgdir}/usr/bin/zeb"
  install -Dm644 zeb.desktop "${pkgdir}/usr/share/applications/zeb.desktop"
  install -Dm644 src-tauri/icons/32x32.png "${pkgdir}/usr/share/icons/hicolor/32x32/apps/zeb.png"
  install -Dm644 src-tauri/icons/128x128.png "${pkgdir}/usr/share/icons/hicolor/128x128/apps/zeb.png"
  install -Dm644 src-tauri/icons/icon.png "${pkgdir}/usr/share/icons/hicolor/512x512/apps/zeb.png"
}
