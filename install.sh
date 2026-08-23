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

# arch
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64) DEB_ARCH="amd64"; RPM_ARCH="x86_64"; APPIMAGE_ARCH="amd64" ;;
  aarch64|arm64) DEB_ARCH="arm64"; RPM_ARCH="aarch64"; APPIMAGE_ARCH="aarch64" ;;
  *) err "unsupported arch: $ARCH" ;;
esac

# os
if [ -f /etc/os-release ]; then . /etc/os-release; OS_ID=$ID; OS_LIKE=$ID_LIKE; else OS_ID="unknown"; fi
info "Detected: $OS_ID ($OS_LIKE) $ARCH"

# deps per distro
install_deps() {
  case "$OS_ID" in
    ubuntu|debian|linuxmint|pop)
      info "Installing deps (apt)..."
      sudo apt-get update -y
      sudo apt-get install -y libwebkit2gtk-4.1-0 libappindicator3-1 librsvg2-common || \
        sudo apt-get install -y libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-common
      ;;
    fedora|rhel|centos|rocky|almalinux)
      info "Installing deps (dnf)..."
      sudo dnf install -y webkit2gtk4.1 libappindicator-gtk3 librsvg2
      ;;
    arch|manjaro|endeavouros|artix)
      info "Installing deps (pacman)..."
      sudo pacman -Sy --noconfirm webkit2gtk-4.1 libappindicator-gtk3 librsvg
      ;;
    opensuse*|suse)
      info "Installing deps (zypper)..."
      sudo zypper install -y webkit2gtk-4.1 libappindicator3-1 librsvg
      ;;
    *) warn "Unknown distro $OS_ID — trying AppImage (no deps needed, just FUSE)";;
  esac
}

# fetch latest tag if needed
if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release..."
  # follow redirect to get tag
  LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "https://github.com/$REPO/releases/latest" || true)
  # fallback to API
  if [ -z "$LATEST_URL" ] || ! echo "$LATEST_URL" | grep -q "/tag/"; then
    TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
  else
    TAG=$(basename "$LATEST_URL")
  fi
  [ -z "$TAG" ] && err "could not resolve latest tag"
  VERSION="$TAG"
  info "Latest: $VERSION"
fi
# strip v
VER_NUM=${VERSION#v}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$TMP"

# try native package first, fallback to AppImage
case "$OS_ID" in
  ubuntu|debian|linuxmint|pop)
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
  fedora|rhel|centos|rocky|almalinux)
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
  arch|manjaro|endeavouros|artix)
    # try AUR first if yay/paru exists, else AppImage
    if command -v yay >/dev/null 2>&1; then
      info "Trying AUR (yay)..."
      yay -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet, falling back"
    elif command -v paru >/dev/null 2>&1; then
      paru -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet"
    fi
    ;;
esac

# universal AppImage fallback (works on any distro with FUSE)
FILE="${APP}_${VER_NUM}_${APPIMAGE_ARCH}.AppImage"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
# try arch-specific, then amd64
info "Downloading $FILE ..."
if ! curl -fL --progress-bar -o "$FILE" "$URL"; then
  FILE="${APP}_${VER_NUM}_amd64.AppImage"
  URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
  info "Trying $FILE ..."
  curl -fL --progress-bar -o "$FILE" "$URL" || err "AppImage not found at $URL — did you push tag $VERSION and wait for release.yml to finish?"
fi

chmod +x "$FILE"
# install to ~/.local/bin and desktop entry
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons"
cp "$FILE" "$HOME/.local/bin/zeb" 2>/dev/null || mv "$FILE" "$HOME/.local/bin/zeb"
cat > "$HOME/.local/share/applications/zeb.desktop" <<DESKTOP
[Desktop Entry]
Name=zebBrowser
Exec=$HOME/.local/bin/zeb
Icon=zeb
Type=Application
Categories=Network;WebBrowser;
Comment=Minimal spotlight browser
DESKTOP
# try to install icon if exists in release
info "Installed AppImage to ~/.local/bin/zeb — ensure ~/.local/bin is in PATH"
info "Run: zeb  or  ~/.local/bin/zeb"

# deps for AppImage (FUSE)
if ! command -v fusermount >/dev/null 2>&1; then
  warn "FUSE may be needed for AppImage: sudo apt install libfuse2 / sudo pacman -S fuse2"
fi
info "Done — $VERSION ($ARCH) on $OS_ID"
