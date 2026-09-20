#!/bin/sh
# AppRun wrapper for Mini Browser AppImage.
#
# Why this exists: when the AppImage is launched from a file manager
# (double-click, "Open With", app drawer), stdout/stderr go nowhere visible.
# If the real binary fails — wrong GTK backend, missing DBus session,
# FUSE refused, etc. — the user sees nothing happen.
#
# This wrapper:
#   1. Sets safe environment defaults (X11 backend, disable DMABUF renderer)
#      so the bundled WebKitGTK 6 works on the widest range of desktops.
#   2. Detects whether it's running under a terminal. If yes, it just execs
#      the real binary so Ctrl+C / shell job control work normally.
#   3. Otherwise (file-manager launch) it logs the environment, ldd output
#      of the binary, and every line of stdout/stderr to
#      ~/.local/share/mini-browser/launch.log so failures are diagnosable.
#
# The real binary is the same one the desktop file's Exec= line points to:
#   <AppDir>/usr/bin/mini-browser

set -eu

# ---- Resolve paths ----------------------------------------------------------
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BIN="$SCRIPT_DIR/usr/bin/mini-browser"

# ---- Safe environment defaults ----------------------------------------------
# Prefer X11 unless the user is already on Wayland with a working compositor.
if [ -z "${WAYLAND_DISPLAY:-}" ] && [ -z "${GDK_BACKEND:-}" ]; then
    if [ -n "${DISPLAY:-}" ]; then
        export GDK_BACKEND=x11
    elif [ -n "${WAYLAND_DISPLAY:-}" ]; then
        export GDK_BACKEND=wayland
    fi
fi

# DMABUF renderer requires working GPU drivers; fall back gracefully on VMs /
# older Intel / Nvidia-without-proprietary. Disable by default — can be
# overridden by the user if they have a known-good GPU setup.
export WEBKIT_DISABLE_DMABUF_RENDERER=${WEBKIT_DISABLE_DMABUF_RENDERER:-1}

# ---- Sanity checks ----------------------------------------------------------
if [ ! -x "$BIN" ]; then
    echo "AppRun: error: $BIN not found or not executable." >&2
    exit 127
fi

# ---- Launch -----------------------------------------------------------------
# A TTY-attached launch (terminal, `./Mini-Browser.AppImage`) just execs the
# binary so signals and exit codes flow through unchanged.
if [ -t 1 ] && [ -t 2 ]; then
    exec "$BIN" "$@"
fi

# Detached launch (file manager, app drawer, .desktop file activation):
# redirect everything to a persistent log so the user can diagnose failures.
LOG_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/mini-browser"
LOG_FILE="$LOG_DIR/launch.log"
if ! mkdir -p "$LOG_DIR" 2>/dev/null; then
    # Fall back to /tmp if the user's home is unwritable (e.g. some kiosk envs).
    LOG_DIR="/tmp/mini-browser-logs"
    mkdir -p "$LOG_DIR" 2>/dev/null || true
    LOG_FILE="$LOG_DIR/launch.log"
fi

{
    echo "============================================================"
    echo "Mini Browser AppRun launch"
    echo "Timestamp:    $(date -Iseconds 2>/dev/null || date)"
    echo "AppImage:     ${APPIMAGE:-<not set>}"
    echo "AppDir:       $SCRIPT_DIR"
    echo "Binary:       $BIN"
    echo "Args:         $*"
    echo "DISPLAY:      ${DISPLAY:-<unset>}"
    echo "WAYLAND_DISPLAY: ${WAYLAND_DISPLAY:-<unset>}"
    echo "XDG_RUNTIME_DIR: ${XDG_RUNTIME_DIR:-<unset>}"
    echo "DBUS_SESSION_BUS_ADDRESS: ${DBUS_SESSION_BUS_ADDRESS:-<unset>}"
    echo "GDK_BACKEND:  ${GDK_BACKEND:-<unset>}"
    echo "WEBKIT_DISABLE_DMABUF_RENDERER: ${WEBKIT_DISABLE_DMABUF_RENDERER:-<unset>}"
    echo "User:         $(id 2>/dev/null || echo unknown)"
    echo "---- ldd ----"
    ldd "$BIN" 2>&1 || true
    echo "---- exec ----"
} >> "$LOG_FILE" 2>&1

# Run the binary as a child so we can record its exit code. `exec` would
# replace this shell and lose the chance to log the result. In terminal mode
# we use exec above so Ctrl+C is delivered straight to the binary; in detached
# mode the file manager doesn't care about signal delivery to its child, and
# capturing the exit code matters more for post-mortem debugging.
set +e
"$BIN" "$@" >> "$LOG_FILE" 2>&1
exit_code=$?
set -e
{
    echo "---- exit code: $exit_code at $(date -Iseconds 2>/dev/null || date) ----"
} >> "$LOG_FILE" 2>&1
exit "$exit_code"