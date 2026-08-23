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
if [ -f /etc/os-release ]; then . /etc/os-release; OS_ID=$ID; OS_LIKE=${ID_LIKE:-}; else OS_ID="unknown"; OS_LIKE=""; fi
info "Detected: $OS_ID ($OS_LIKE) $ARCH"

# deps per distro (handles cachyos via ID_LIKE)
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
    *) warn "Unknown distro $OS_ID — trying AppImage (no deps needed, just FUSE)";;
  esac
}

# fetch latest tag if needed — handle no release yet
if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release..."
  LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "https://github.com/$REPO/releases/latest" 2>/dev/null || true)
  TAG=""
  if [ -n "$LATEST_URL" ] && echo "$LATEST_URL" | grep -q "/tag/"; then
    TAG=$(basename "$LATEST_URL")
  else
    # try GitHub API, but don't fail on 404
    API_RESP=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null || true)
    TAG=$(echo "$API_RESP" | grep '"tag_name"' | cut -d'"' -f4 || true)
  fi
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    echo ""
    warn "No prebuilt release found for $REPO yet."
    echo -e "  This usually means no GitHub Release has been published.\n"
    echo -e "  Options:\n"
    echo -e "  1) Wait for the first release — maintainer needs to run:\n     ${GREEN}git tag v0.1.0 && git push origin v0.1.0${NC}  (triggers release.yml)\n"
    echo -e "  2) Or build from source now:\n     ${GREEN}git clone https://github.com/$REPO.git && cd $(basename $REPO) && pnpm install && pnpm tauri build${NC}\n"
    # fallback to building from source if pnpm/cargo available
    if command -v pnpm >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
      warn "Attempting to build from source..."
      TMP_SRC=$(mktemp -d)
      git clone "https://github.com/$REPO.git" "$TMP_SRC/repo" 2>/dev/null || err "failed to clone $REPO"
      cd "$TMP_SRC/repo"
      pnpm install
      pnpm tauri build
      info "Build finished — binaries in src-tauri/target/release/bundle/"
      info "You can run: ./src-tauri/target/release/zeb  or install the bundle"
      exit 0
    else
      err "No release and no pnpm/cargo found to build from source. Please install pnpm and Rust, or wait for a release."
    fi
  fi
  VERSION="$TAG"
  info "Latest: $VERSION"
fi
# strip v
VER_NUM=${VERSION#v}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$TMP"

# try native package first, fallback to AppImage
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
    if command -v yay >/dev/null 2>&1; then
      info "Trying AUR (yay)..."
      yay -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet, falling back"
    elif command -v paru >/dev/null 2>&1; then
      paru -S --noconfirm zeb-browser 2>/dev/null && exit 0 || warn "AUR not yet"
    fi
    ;;
esac

# universal AppImage fallback
FILE="${APP}_${VER_NUM}_${APPIMAGE_ARCH}.AppImage"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
info "Downloading $FILE ..."
if ! curl -fL --progress-bar -o "$FILE" "$URL"; then
  FILE="${APP}_${VER_NUM}_amd64.AppImage"
  URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
  info "Trying $FILE ..."
  curl -fL --progress-bar -o "$FILE" "$URL" || err "AppImage not found at $URL — did you push tag $VERSION and wait for release.yml to finish? Check https://github.com/$REPO/releases"
fi

chmod +x "$FILE"
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
info "Installed AppImage to ~/.local/bin/zeb — ensure ~/.local/bin is in PATH"
info "Run: zeb  or  ~/.local/bin/zeb"
if ! command -v fusermount >/dev/null 2>&1; then
  warn "FUSE may be needed for AppImage: sudo apt install libfuse2 / sudo pacman -S fuse2"
fi
info "Done — $VERSION ($ARCH) on $OS_ID"
