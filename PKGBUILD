# Maintainer: sahilcodexx <sahil171759@gmail.com>
# Build: makepkg -si  (from repo root)
# Builds Electron Mini Browser from source — no AppImage wrapping, native Wayland-friendly.

pkgname=mini-browser
pkgver=1.0.0
pkgrel=1
pkgdesc="Minimal Electron browser — floating Zen pill, no tabs/bookmarks/history"
arch=('x86_64' 'aarch64')
url="https://github.com/sahilcodexx/zebBrowser"
license=('GPL-3.0-only')
depends=('gtk3' 'nss' 'alsa-lib' 'libxss' 'libxtst' 'libxcb' 'libdrm' 'at-spi2-core' 'cairo' 'pango' 'hicolor-icon-theme')
makedepends=('nodejs' 'npm' 'pnpm')
provides=('mini-browser')
conflicts=('mini-browser')
source=()
sha256sums=()
options=('!strip' '!debug')

build() {
  cd "$startdir"
  # clean electron cache confusions on Arch
  export ELECTRON_SKIP_BINARY_DOWNLOAD=0
  # use pnpm if available, fallback to npm
  if command -v pnpm >/dev/null 2>&1; then
    pnpm install --frozen-lockfile || pnpm install
    # build unpacked dir (no AppImage) for packaging
    npx electron-builder --linux --dir --config.directories.output=dist 2>&1 | tee build.log
  else
    npm install
    npx electron-builder --linux --dir --config.directories.output=dist
  fi
}

package() {
  cd "$startdir"

  # dist/linux-unpacked is created by electron-builder --dir
  # fallback to dist if versioned subdir
  _unpacked="dist/linux-unpacked"
  if [ ! -d "$_unpacked" ]; then
    _unpacked=$(find dist -maxdepth 1 -type d -name "linux-unpacked" | head -n1)
  fi
  if [ -z "$_unpacked" ] || [ ! -d "$_unpacked" ]; then
    echo "error: linux-unpacked not found after build. Contents of dist:"
    ls -R dist 2>&1 | head -100
    return 1
  fi

  install -d "$pkgdir/opt/$pkgname"
  cp -r "$_unpacked"/* "$pkgdir/opt/$pkgname/"

  # wrapper in /usr/bin
  install -Dm755 /dev/stdin "$pkgdir/usr/bin/$pkgname" <<WRAPPER
#!/bin/sh
exec /opt/$pkgname/mini-browser "\$@"
WRAPPER

  # also provide 'mini' short command
  ln -s "/opt/$pkgname/mini-browser" "$pkgdir/usr/bin/mini"

  # icons
  install -Dm644 build/icon.png "$pkgdir/usr/share/icons/hicolor/512x512/apps/$pkgname.png"
  for s in 16 32 48 64 128 256; do
    # electron-builder already generates icons in build/icons, but ensure at least 512 exists
    if [ -f "build/icons/${s}x${s}.png" ]; then
      install -Dm644 "build/icons/${s}x${s}.png" "$pkgdir/usr/share/icons/hicolor/${s}x${s}/apps/$pkgname.png"
    fi
  done

  # desktop entry
  install -Dm644 mini-browser.desktop "$pkgdir/usr/share/applications/$pkgname.desktop"
  # license
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE" 2>/dev/null || true
}
