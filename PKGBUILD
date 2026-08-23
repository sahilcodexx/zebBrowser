# Maintainer: sahilcodexx <sahil171759@gmail.com>
pkgname=zeb-browser
pkgver=0.1.0
pkgrel=1
pkgdesc="Minimal spotlight browser — centered pill, hover Zeb bar, Tauri"
arch=('x86_64' 'aarch64')
url="https://github.com/sahilcodexx/zebBrowser"
license=('MIT')
depends=('webkit2gtk-4.1' 'libappindicator-gtk3' 'librsvg' 'cairo' 'gtk3')
source_x86_64=("${pkgname}-${pkgver}-x86_64.AppImage::https://github.com/sahilcodexx/zebBrowser/releases/download/v${pkgver}/zeb_${pkgver}_amd64.AppImage")
source_aarch64=("${pkgname}-${pkgver}-aarch64.AppImage::https://github.com/sahilcodexx/zebBrowser/releases/download/v${pkgver}/zeb_${pkgver}_aarch64.AppImage")
sha256sums_x86_64=('SKIP')
sha256sums_aarch64=('SKIP')
package() {
  install -Dm755 "${srcdir}/${pkgname}-${pkgver}-${CARCH}.AppImage" "${pkgdir}/usr/bin/zeb"
  install -Dm644 /dev/null "${pkgdir}/usr/share/applications/zeb.desktop"
  cat > "${pkgdir}/usr/share/applications/zeb.desktop" <<DESKTOP
[Desktop Entry]
Name=zebBrowser
Exec=/usr/bin/zeb
Icon=zeb
Type=Application
Categories=Network;WebBrowser;
DESKTOP
}
