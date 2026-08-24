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
      info "Installing dependencies (apt)..."
      sudo apt-get update -y
      sudo apt-get install -y libwebkit2gtk-4.1-0 libappindicator3-1 librsvg2-common libfuse2 curl || \
        sudo apt-get install -y libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-common libfuse2 curl
      ;;
    *fedora*|*rhel*|*centos*|*rocky*|*almalinux*)
      info "Installing dependencies (dnf)..."
      sudo dnf install -y webkit2gtk4.1 libappindicator-gtk3 librsvg2 fuse-libs curl
      ;;
    *arch*|*manjaro*|*endeavouros*|*artix*|*cachyos*)
      info "Installing dependencies (pacman)..."
      sudo pacman -Sy --noconfirm --needed webkit2gtk-4.1 libappindicator-gtk3 librsvg fuse2 curl
      ;;
    *opensuse*|*suse*)
      info "Installing dependencies (zypper)..."
      sudo zypper install -y webkit2gtk-4.1 libappindicator3-1 librsvg libfuse2 curl
      ;;
    *) warn "Unknown distro $OS_ID — attempting install";;
  esac
}

# helper to install AppImage with Wayland fix wrapper
install_appimage() {
  local file="$1" url="$2"
  info "Downloading $file from $url ..."
  curl -fL --progress-bar -o "$file" "$url" || return 1
  chmod +x "$file"
  mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"

  mv "$file" "$HOME/.local/bin/zeb.AppImage"

  # download icon
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/src-tauri/icons/icon.png" -o "$HOME/.local/share/icons/hicolor/512x512/apps/zeb.png" 2>/dev/null || true

  # wrapper handles host Wayland / Mesa library preload for WebKitGTK
  cat > "$HOME/.local/bin/zeb" <<'WRAPPER'
#!/usr/bin/env bash
# Auto-detect host libwayland-client to prevent WebKitGTK EGL crashes on modern Wayland / Mesa compositors
for lib in /usr/lib/libwayland-client.so.0 /usr/lib/x86_64-linux-gnu/libwayland-client.so.0 /usr/lib64/libwayland-client.so.0 /usr/lib/aarch64-linux-gnu/libwayland-client.so.0; do
  if [ -f "$lib" ]; then
    export LD_PRELOAD="${LD_PRELOAD:+$LD_PRELOAD:}$lib"
    break
  fi
done

exec "$HOME/.local/bin/zeb.AppImage" "$@"
WRAPPER
  chmod +x "$HOME/.local/bin/zeb"

  cat > "$HOME/.local/share/applications/zeb.desktop" <<DESKTOP
[Desktop Entry]
Name=zeb
Comment=Minimal spotlight browser
Exec=$HOME/.local/bin/zeb
Icon=zeb
Terminal=false
Type=Application
Categories=Network;WebBrowser;
StartupWMClass=zeb
DESKTOP
  chmod +x "$HOME/.local/share/applications/zeb.desktop"

  info "Installed AppImage to ~/.local/bin/zeb (with Wayland EGL compatibility wrapper)"
  info "Make sure ~/.local/bin is in your PATH. Run: zeb"
  return 0
}

if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release..."
  API_RESP=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null || true)
  TAG=$(echo "$API_RESP" | grep '"tag_name"' | cut -d'"' -f4 || true)
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "https://github.com/$REPO/releases/latest" 2>/dev/null || true)
    if [ -n "$LATEST_URL" ] && echo "$LATEST_URL" | grep -q "/tag/"; then
      TAG=$(basename "$LATEST_URL")
    fi
  fi
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    warn "No prebuilt GitHub Release tag found for $REPO yet."
    if command -v cargo >/dev/null 2>&1 && (command -v pnpm >/dev/null 2>&1 || command -v npm >/dev/null 2>&1); then
      info "Building natively from source..."
      install_deps
      TMP_SRC=$(mktemp -d)
      git clone --depth 1 "https://github.com/$REPO.git" "$TMP_SRC/repo" 2>/dev/null || err "failed to clone $REPO"
      cd "$TMP_SRC/repo"
      if command -v pnpm >/dev/null 2>&1; then
        pnpm install
        pnpm build
      else
        npx --yes pnpm install || npm install
        npx --yes pnpm build || npm run build
      fi
      unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS RUSTFLAGS
      cargo build --release --manifest-path src-tauri/Cargo.toml
      mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
      install -Dm755 src-tauri/target/release/zeb "$HOME/.local/bin/zeb"
      install -Dm644 zeb.desktop "$HOME/.local/share/applications/zeb.desktop"
      install -Dm644 src-tauri/icons/icon.png "$HOME/.local/share/icons/hicolor/512x512/apps/zeb.png"
      info "Installed successfully from source to ~/.local/bin/zeb — run: zeb"
      exit 0
    else
      err "No prebuilt release found and missing cargo/node build tools."
    fi
  fi
  VERSION="$TAG"
  info "Latest release version: $VERSION"
fi
VER_NUM=${VERSION#zeb-v}; VER_NUM=${VER_NUM#v}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$TMP"

case "$OS_ID $OS_LIKE" in
  *ubuntu*|*debian*|*linuxmint*|*pop*)
    FILE="${APP}_${VER_NUM}_${DEB_ARCH}.deb"
    URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
    info "Attempting deb download from $URL ..."
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
    info "Attempting rpm download from $URL ..."
    if curl -fL --progress-bar -o "$FILE" "$URL"; then
      install_deps
      info "Installing $FILE ..."
      sudo rpm -i "$FILE" || sudo dnf install -y "$FILE"
      info "Installed via rpm — run: zeb"
      exit 0
    else
      warn "rpm not found, falling back to AppImage"
    fi
    ;;
  *arch*|*cachyos*|*manjaro*|*endeavouros*|*artix*)
    # On Arch/CachyOS, install dependencies and try native PKGBUILD first for optimal Wayland performance
    install_deps
    if [ -f "/usr/bin/makepkg" ] && command -v cargo >/dev/null 2>&1; then
      info "Arch Linux detected — building natively via PKGBUILD..."
      TMP_SRC2=$(mktemp -d)
      if git clone --depth 1 "https://github.com/$REPO.git" "$TMP_SRC2/repo" 2>/dev/null; then
        cd "$TMP_SRC2/repo"
        if makepkg -si --noconfirm; then
          info "Successfully installed via PKGBUILD — run: zeb"
          exit 0
        else
          warn "PKGBUILD build failed, falling back to AppImage"
        fi
      fi
    fi
    ;;
esac

# AppImage installation
install_deps
FILE="${APP}_${VER_NUM}_${APPIMAGE_ARCH}.AppImage"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
if ! install_appimage "$FILE" "$URL"; then
  FILE="${APP}_${VER_NUM}_amd64.AppImage"
  URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"
  install_appimage "$FILE" "$URL" || err "AppImage not found at $URL. Please check https://github.com/$REPO/releases"
fi

info "Installation complete! Run: zeb"
