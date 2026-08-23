#!/usr/bin/env bash
set -e
# zebBrowser universal installer — any arch, any distro
# curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
REPO="sahilcodexx/zebBrowser"
APP="zeb"
VERSION="${VERSION:-latest}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info() { echo -e "${GREEN}==>${NC} $*"; }
warn() { echo -e "${YELLOW}warn:${NC} $*"; }
err() { echo -e "${RED}error:${NC} $*"; exit 1; }

ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64) DEB_ARCH="amd64"; RPM_ARCH="x86_64"; APPIMAGE_ARCH="amd64" ;;
  aarch64|arm64) DEB_ARCH="arm64"; RPM_ARCH="aarch64"; APPIMAGE_ARCH="aarch64" ;;
  *) err "unsupported arch: $ARCH" ;;
esac

if [ -f /etc/os-release ]; then . /etc/os-release; OS_ID=$ID; OS_LIKE=${ID_LIKE:-}; else OS_ID="unknown"; OS_LIKE=""; fi
info "Detected: $OS_ID ($OS_LIKE) $ARCH"

install_deps() {
  local id="$OS_ID $OS_LIKE"
  case "$id" in
    *ubuntu*|*debian*|*linuxmint*|*pop*)
      info "Installing deps (apt)..."
      sudo apt-get update -y
      sudo apt-get install -y libwebkit2gtk-4.1-0 libappindicator3-1 librsvg2-common || \
        sudo apt-get install -y libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-common
      ;;
    *fedora*|*rhel*|*centos*|*rocky*|*almalinux*)
      info "Installing deps (dnf)..."
      sudo dnf install -y webkit2gtk4.1 libappindicator-gtk3 librsvg2
      ;;
    *arch*|*manjaro*|*endeavouros*|*artix*|*cachyos*)
      info "Installing deps (pacman)..."
      sudo pacman -Sy --noconfirm webkit2gtk-4.1 libappindicator-gtk3 librsvg
      ;;
    *opensuse*|*suse*)
      info "Installing deps (zypper)..."
      sudo zypper install -y webkit2gtk-4.1 libappindicator3-1 librsvg
      ;;
    *) warn "Unknown distro $OS_ID — trying AppImage";;
  esac
}

# helper to install AppImage with Wayland fix wrapper
install_appimage() {
  local file="$1" url="$2"
  info "Downloading $file ..."
  curl -fL --progress-bar -o "$file" "$URL" || return 1
  chmod +x "$file"
  mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
  # keep real binary as .AppImage, wrapper handles Wayland EGL fix for Hyprland
  mv "$file" "$HOME/.local/bin/zeb.AppImage"
  cat > "$HOME/.local/bin/zeb" <<'WRAPPER'
#!/usr/bin/env bash
# Hyprland/Wayland EGL fix — try X11 via XWayland, fall back to software
export GDK_BACKEND=x11
export WAYLAND_DISPLAY=""
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export LIBGL_ALWAYS_SOFTWARE=1
export EGL_PLATFORM=x11
exec "$HOME/.local/bin/zeb.AppImage" "$@"
WRAPPER
  chmod +x "$HOME/.local/bin/zeb"
  cat > "$HOME/.local/share/applications/zeb.desktop" <<DESKTOP
[Desktop Entry]
Name=zebBrowser
Exec=$HOME/.local/bin/zeb
Icon=zeb
Type=Application
Categories=Network;WebBrowser;
Comment=Minimal spotlight browser
DESKTOP
  info "Installed AppImage to ~/.local/bin/zeb (wrapper handles Wayland) — ensure ~/.local/bin is in PATH"
  info "Run: zeb"
  return 0
}

if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release..."
  # prefer API (not CDN-cached) — handles just-published releases
  API_RESP=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null || true)
  TAG=$(echo "$API_RESP" | grep '"tag_name"' | cut -d'"' -f4 || true)
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "https://github.com/$REPO/releases/latest" 2>/dev/null || true)
    if [ -n "$LATEST_URL" ] && echo "$LATEST_URL" | grep -q "/tag/"; then
      TAG=$(basename "$LATEST_URL")
    fi
  fi
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    echo ""
    warn "No prebuilt release found for $REPO yet."
    echo -e "  Options:\n"
    echo -e "  1) Wait for first release: ${GREEN}git tag v0.1.3 && git push origin v0.1.3${NC}\n"
    echo -e "  2) Or build from source now:\n     ${GREEN}git clone https://github.com/$REPO.git && cd $(basename $REPO) && pnpm install && pnpm tauri build --bundles deb${NC}\n"
    if command -v pnpm >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
      warn "Attempting to build from source (deb only, avoids AppImage EGL)..."
      TMP_SRC=$(mktemp -d)
      git clone "https://github.com/$REPO.git" "$TMP_SRC/repo" 2>/dev/null || err "failed to clone $REPO"
      cd "$TMP_SRC/repo"
      pnpm install
      TAURI_SIGNING_PRIVATE_KEY="" pnpm tauri build --bundles deb
      info "Build finished — install with: sudo dpkg -i src-tauri/target/release/bundle/deb/*.deb  or  sudo pacman -U"
      exit 0
    else
      err "No release and no pnpm/cargo to build from source."
    fi
  fi
  VERSION="$TAG"
  info "Latest: $VERSION"
fi
VER_NUM=${VERSION#zeb-v}; VER_NUM=${VER_NUM#v}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$TMP"

case "$OS_ID $OS_LIKE" in
  *ubuntu*|*debian*|*linuxmint*|*pop*)
    FILE="${APP}_${VER_NUM}_${DEB_ARCH}.deb"
    URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
    info "Downloading $FILE ..."
    if curl -fL --progress-bar -o "$FILE" "$URL"; then
      install_deps
      info "Installing $FILE ..."
      sudo dpkg -i "$FILE" || sudo apt-get install -f -y
      info "Installed via deb — run: zeb"
      exit 0
    else
      warn "deb not found, falling back to AppImage"
    fi
    ;;
  *fedora*|*rhel*|*centos*|*rocky*|*almalinux*)
    FILE="${APP}-${VER_NUM}-1.${RPM_ARCH}.rpm"
    URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
    info "Downloading $FILE ..."
    if curl -fL --progress-bar -o "$FILE" "$URL"; then
      install_deps
      sudo rpm -i "$FILE" || sudo dnf install -y "$FILE"
      info "Installed via rpm — run: zeb"
      exit 0
    else
      warn "rpm not found, falling back to AppImage"
    fi
    ;;
  *arch*|*cachyos*|*manjaro*|*endeavouros*|*artix*)
    # prefer native build via PKGBUILD on Arch (avoids AppImage EGL on Hyprland)
    if [ -f "/usr/bin/pacman" ] && command -v pnpm >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
      info "Arch detected — trying native PKGBUILD build (better than AppImage on Wayland)..."
      TMP_SRC2=$(mktemp -d)
      git clone --depth 1 "https://github.com/$REPO.git" "$TMP_SRC2/repo" 2>/dev/null && \
      cd "$TMP_SRC2/repo" && makepkg -si --noconfirm 2>/dev/null && info "Installed via PKGBUILD — run: zeb" && exit 0 || warn "PKGBUILD build failed, falling back to AppImage"
    fi
    if command -v yay >/dev/null 2>&1; then
      info "Trying AUR (yay)..."
      yay -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet, falling back"
    elif command -v paru >/dev/null 2>&1; then
      paru -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet"
    fi
    ;;
esac

# AppImage fallback with Wayland wrapper
FILE="${APP}_${VER_NUM}_${APPIMAGE_ARCH}.AppImage"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
if ! install_appimage "$FILE" "$URL"; then
  FILE="${APP}_${VER_NUM}_amd64.AppImage"
  URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
  install_appimage "$FILE" "$URL" || err "AppImage not found at $URL — did you push tag $VERSION and wait for release.yml? Check https://github.com/$REPO/releases"
fi
if ! command -v fusermount >/dev/null 2>&1; then
  warn "FUSE may be needed: sudo pacman -S fuse2 / sudo apt install libfuse2"
fi
info "Done — $VERSION ($ARCH) on $OS_ID (with Wayland fix wrapper)"
