#!/usr/bin/env bash
set -e
# Mini Browser — universal installer (AppImage, any distro/arch)
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
#   VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/sahilcodexx/zebBrowser/main/install.sh | bash
#   ./install.sh  (from cloned repo)

REPO="sahilcodexx/zebBrowser"
APP="mini-browser"
BIN_NAME="mini-browser"
VERSION="${VERSION:-latest}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info() { echo -e "${GREEN}==>${NC} $*"; }
warn() { echo -e "${YELLOW}warn:${NC} $*"; }
err() { echo -e "${RED}error:${NC} $*"; exit 1; }

# Detect arch
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64) ARCH_NORM="x64"; DEB_ARCH="amd64" ;;
  aarch64|arm64) ARCH_NORM="arm64"; DEB_ARCH="arm64" ;;
  *) err "unsupported arch: $ARCH (only x86_64 and aarch64)" ;;
esac

# Detect OS for deps
if [ -f /etc/os-release ]; then . /etc/os-release; OS_ID=$ID; OS_LIKE=${ID_LIKE:-}; else OS_ID="unknown"; OS_LIKE=""; fi
info "Detected: $OS_ID ($OS_LIKE) $ARCH"

install_deps() {
  local id="$OS_ID $OS_LIKE"
  case "$id" in
    *ubuntu*|*debian*|*linuxmint*|*pop*)
      info "Installing AppImage deps (apt)..."
      sudo apt-get update -y
      # libfuse2 needed for AppImage on Ubuntu 22.04+, libfuse3 on newer but 2 is compat
      sudo apt-get install -y libfuse2 curl file || sudo apt-get install -y fuse curl file || true
      ;;
    *fedora*|*rhel*|*centos*|*rocky*|*almalinux*)
      info "Installing deps (dnf)..."
      sudo dnf install -y fuse-libs curl file || sudo dnf install -y fuse curl file || true
      ;;
    *arch*|*manjaro*|*endeavouros*|*artix*|*cachyos*)
      info "Installing deps (pacman)..."
      sudo pacman -Sy --noconfirm --needed fuse2 curl file 2>/dev/null || sudo pacman -Sy --noconfirm --needed fuse curl file || true
      ;;
    *opensuse*|*suse*)
      info "Installing deps (zypper)..."
      sudo zypper install -y libfuse2 curl file || true
      ;;
    *) warn "Unknown distro $OS_ID — skipping dep install (AppImage is self-contained, just needs FUSE)" ;;
  esac
}

install_from_source() {
  info "No prebuilt release found — building from source..."
  if ! command -v node >/dev/null 2>&1; then err "node not found. Install nodejs 20+ first."; fi
  # prefer pnpm, fallback to npm
  if command -v pnpm >/dev/null 2>&1; then PKG="pnpm"; else PKG="npm"; fi
  TMP_SRC=$(mktemp -d)
  trap 'rm -rf "$TMP_SRC"' EXIT
  git clone --depth 1 "https://github.com/$REPO.git" "$TMP_SRC/repo" || err "failed to clone $REPO"
  cd "$TMP_SRC/repo"
  if [ "$PKG" = "pnpm" ]; then
    pnpm install --frozen-lockfile || pnpm install
    pnpm run build
    # the built AppImage will be in dist/*.AppImage
    APPIMAGE=$(find dist -maxdepth 1 -name "*.AppImage" | head -n1)
    if [ -z "$APPIMAGE" ]; then err "build succeeded but no AppImage in dist/"; fi
    info "Built $APPIMAGE — installing..."
    mkdir -p "$HOME/.local/bin"
    cp "$APPIMAGE" "$HOME/.local/bin/${BIN_NAME}.AppImage"
    chmod +x "$HOME/.local/bin/${BIN_NAME}.AppImage"
  else
    npm install
    npm run build
    APPIMAGE=$(find dist -maxdepth 1 -name "*.AppImage" | head -n1)
    [ -z "$APPIMAGE" ] && err "no AppImage after build"
    mkdir -p "$HOME/.local/bin"
    cp "$APPIMAGE" "$HOME/.local/bin/${BIN_NAME}.AppImage"
    chmod +x "$HOME/.local/bin/${BIN_NAME}.AppImage"
  fi
  # fallthrough to wrapper/desktop creation
}

# Resolve latest tag if needed
if [ "$VERSION" = "latest" ]; then
  info "Resolving latest release..."
  API_URL="https://api.github.com/repos/$REPO/releases/latest"
  API_RESP=$(curl -fsSL "$API_URL" 2>/dev/null || true)
  if command -v jq >/dev/null 2>&1 && [ -n "$API_RESP" ]; then
    TAG=$(echo "$API_RESP" | jq -r '.tag_name // empty' 2>/dev/null || true)
    ASSET_URL=$(echo "$API_RESP" | jq -r '.assets[] | select(.name | test("AppImage"; "i")) | .browser_download_url' 2>/dev/null | head -n1 || true)
  else
    TAG=$(echo "$API_RESP" | grep -m1 '"tag_name"' | cut -d'"' -f4 2>/dev/null || true)
    ASSET_URL=$(echo "$API_RESP" | grep -o '"browser_download_url": *"[^"]*AppImage[^"]*"' | head -n1 | cut -d'"' -f4 2>/dev/null || true)
  fi
  if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    # fallback to redirect
    LATEST_URL=$(curl -fsSL -o /dev/null -w "%{url_effective}" "https://github.com/$REPO/releases/latest" 2>/dev/null || true)
    if [ -n "$LATEST_URL" ] && echo "$LATEST_URL" | grep -q "/tag/"; then
      TAG=$(basename "$LATEST_URL")
    fi
  fi
  if [ -n "$TAG" ] && [ "$TAG" != "null" ]; then
    VERSION="$TAG"
    info "Latest: $VERSION"
    # if ASSET_URL already found, keep it; else resolve via tag API
    if [ -z "$ASSET_URL" ] || [ "$ASSET_URL" = "null" ]; then
      TAG_API="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
      TAG_RESP=$(curl -fsSL "$TAG_API" 2>/dev/null || true)
      if command -v jq >/dev/null 2>&1; then
        ASSET_URL=$(echo "$TAG_RESP" | jq -r '.assets[] | select(.name | test("AppImage"; "i")) | .browser_download_url' 2>/dev/null | head -n1 || true)
      else
        ASSET_URL=$(echo "$TAG_RESP" | grep -o '"browser_download_url": *"[^"]*AppImage[^"]*"' | head -n1 | cut -d'"' -f4 2>/dev/null || true)
      fi
    fi
  else
    warn "No release tag found."
    ASSET_URL=""
  fi
else
  # version pinned, try to get asset URL for that tag
  info "Requested version: $VERSION"
  TAG_API="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
  TAG_RESP=$(curl -fsSL "$TAG_API" 2>/dev/null || true)
  if command -v jq >/dev/null 2>&1; then
    ASSET_URL=$(echo "$TAG_RESP" | jq -r '.assets[] | select(.name | test("AppImage"; "i")) | .browser_download_url' 2>/dev/null | head -n1 || true)
  else
    ASSET_URL=$(echo "$TAG_RESP" | grep -o '"browser_download_url": *"[^"]*AppImage[^"]*"' | head -n1 | cut -d'"' -f4 2>/dev/null || true)
  fi
  # fallback to constructed URL (common naming)
  if [ -z "$ASSET_URL" ] || [ "$ASSET_URL" = "null" ]; then
    VER_NUM=${VERSION#v}; VER_NUM=${VER_NUM#zeb-v}
    # try common patterns
    ASSET_URL="https://github.com/$REPO/releases/download/$VERSION/Mini Browser-${VER_NUM}.AppImage"
  fi
fi

# If we still have no ASSET_URL, try to build from source
if [ -z "$ASSET_URL" ] || [ "$ASSET_URL" = "null" ]; then
  warn "Could not find AppImage asset for $VERSION"
  install_deps
  install_from_source
else
  info "Asset: $ASSET_URL"
  install_deps
  TMP=$(mktemp -d)
  trap 'rm -rf "$TMP"' EXIT
  cd "$TMP"
  info "Downloading AppImage..."
  # -L follow redirect, --progress-bar if tty
  if ! curl -fL --progress-bar -o "${BIN_NAME}.AppImage" "$ASSET_URL"; then
    warn "Failed to download AppImage via API URL, trying fallback..."
    # fallback: try spaced and dashed names
    VER_NUM=${VERSION#v}; VER_NUM=${VER_NUM#zeb-v}
    for cand in \
      "https://github.com/$REPO/releases/download/$VERSION/Mini Browser-${VER_NUM}.AppImage" \
      "https://github.com/$REPO/releases/download/$VERSION/mini-browser-${VER_NUM}.AppImage" \
      "https://github.com/$REPO/releases/download/$VERSION/Mini_Browser-${VER_NUM}.AppImage"
    do
      info "Trying $cand"
      if curl -fL --progress-bar -o "${BIN_NAME}.AppImage" "$cand"; then
        ASSET_URL="$cand"
        break
      fi
    done
    [ -f "${BIN_NAME}.AppImage" ] || {
      warn "Download failed, falling back to source build..."
      install_from_source
      # if source build succeeded, skip download block
      ASSET_URL="built"
    }
  fi

  if [ -f "${BIN_NAME}.AppImage" ]; then
    chmod +x "${BIN_NAME}.AppImage"
    mkdir -p "$HOME/.local/bin"
    mv "${BIN_NAME}.AppImage" "$HOME/.local/bin/${BIN_NAME}.AppImage"
    info "Installed AppImage to ~/.local/bin/${BIN_NAME}.AppImage"
  fi
fi

# Create wrapper and desktop entry (works for both download and source build)
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"

# Try to install icon from repo (for AppImage case, icon is inside AppImage but we want a file for desktop)
if [ ! -f "$HOME/.local/share/icons/hicolor/512x512/apps/${BIN_NAME}.png" ]; then
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/build/icon.png" -o "$HOME/.local/share/icons/hicolor/512x512/apps/${BIN_NAME}.png" 2>/dev/null || \
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/minilogo.png" -o "$HOME/.local/share/icons/hicolor/512x512/apps/${BIN_NAME}.png" 2>/dev/null || true
fi

cat > "$HOME/.local/bin/${BIN_NAME}" <<WRAPPER
#!/usr/bin/env bash
# Mini Browser wrapper — handles Wayland/Mesa quirks
# Set ELECTRON_DISABLE_GPU=1 to force software render if you see GPU errors on Hyprland
if [ "\$ELECTRON_DISABLE_GPU" = "1" ]; then
  exec "\$HOME/.local/bin/${BIN_NAME}.AppImage" --disable-gpu "\$@"
fi
exec "\$HOME/.local/bin/${BIN_NAME}.AppImage" "\$@"
WRAPPER
chmod +x "$HOME/.local/bin/${BIN_NAME}"

cat > "$HOME/.local/share/applications/${BIN_NAME}.desktop" <<DESKTOP
[Desktop Entry]
Name=Mini Browser
Comment=Minimal Electron browser — floating Zen pill
Exec=$HOME/.local/bin/${BIN_NAME} %U
Icon=${BIN_NAME}
Terminal=false
Type=Application
Categories=Network;WebBrowser;
StartupWMClass=mini-browser
MimeType=x-scheme-handler/http;x-scheme-handler/https;text/html;
DESKTOP
chmod +x "$HOME/.local/share/applications/${BIN_NAME}.desktop"

# Update desktop DB if available
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi

# Ensure ~/.local/bin in PATH
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
  warn "~/.local/bin is not in PATH — add to your shell rc:"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

info "Installed successfully! Run: ${BIN_NAME}"
info "  ${BIN_NAME}                 # open home grid"
info "  ${BIN_NAME} https://example.com"
info "Tip: if you see GPU errors on Hyprland, run: ELECTRON_DISABLE_GPU=1 ${BIN_NAME}"
