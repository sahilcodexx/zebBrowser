#!/bin/sh
# AppRun wrapper for Mini Browser AppImage.
#
# Four problems this wrapper solves:
#
# 1. WebKitGTK 6 multiarch helper path
#    The bundled libwebkitgtk-6.0.so.4 (Ubuntu 24.04 build) spawns its helper
#    processes from the Debian multiarch path
#    /usr/lib/x86_64-linux-gnu/webkitgtk-6.0/. We bundle those helpers in
#    $APPDIR/usr/lib/x86_64-linux-gnu/webkitgtk-6.0/ (with patched RPATH so
#    they find their bundled libs). If the host already has the system helpers,
#    they're used directly; otherwise the bundled copies are used via an
#    LD_PRELOAD shim that rewrites execve/execv/execvp/execvpe/posix_spawn.
#
# 2. WebKitGTK 6 sandbox (bwrap)
#    WebKitGTK tries to launch helpers via /usr/bin/bwrap to sandbox them. The
#    sandbox would not be able to see the AppDir's bundled libs, so we disable
#    it via WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 (WebKit's official knob).
#    The browser runs without a per-process sandbox; this is the same tradeoff
#    every flatpak / AppImage WebKitGTK 6 browser makes.
#
# 3. Silent launch from a file manager
#    When launched from a file manager with no terminal attached, every
#    output of the real binary goes nowhere visible. We redirect output to
#    ~/.local/share/mini-browser/launch.log so failures are diagnosable.
#
# 4. Missing environment
#    We set GDK_BACKEND (x11 or wayland) and
#    WEBKIT_DISABLE_DMABUF_RENDERER=1 so the bundled WebKitGTK 6 works on
#    the widest range of desktops.
#
# TTY-attached launches (running from a terminal) take a fast path: we just
# exec the real binary so signals and exit codes flow through unchanged.

set -eu

# ---- Resolve paths ----------------------------------------------------------
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BIN="$SCRIPT_DIR/usr/bin/mini-browser"
HELPERS_SRC="$SCRIPT_DIR/usr/lib/x86_64-linux-gnu/webkitgtk-6.0"
HELPERS_DST="/usr/lib/x86_64-linux-gnu/webkitgtk-6.0"

# ---- Safe environment defaults ----------------------------------------------
if [ -z "${GDK_BACKEND:-}" ]; then
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
        export GDK_BACKEND=wayland
    elif [ -n "${DISPLAY:-}" ]; then
        export GDK_BACKEND=x11
    fi
fi
export WEBKIT_DISABLE_DMABUF_RENDERER=${WEBKIT_DISABLE_DMABUF_RENDERER:-1}

# Disable WebKitGTK 6's bubblewrap sandbox. The sandbox would need access to
# the AppDir's bundled libraries via bind-mounts that we can't easily arrange
# without root. The user-visible effect is no per-process sandboxing; the
# browser still inherits the user's normal Unix permissions. Users can
# override by unsetting this in their environment.
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=${WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS:-1}

# Expose AppDir so the LD_PRELOAD shim (if loaded below) can find the helpers.
export APPDIR="$SCRIPT_DIR"

# Pre-load the redirect shim if it exists. It intercepts execve/execv/execvp/
# execvpe/posix_spawn in the parent and rewrites paths under
# /usr/lib/x86_64-linux-gnu/webkitgtk-6.0/ to point at the bundled helpers.
# No-op on hosts that already have those paths.
if [ -f "$SCRIPT_DIR/usr/lib/libwebkit-helper-redirect.so" ]; then
    export LD_PRELOAD="$SCRIPT_DIR/usr/lib/libwebkit-helper-redirect.so${LD_PRELOAD:+:$LD_PRELOAD}"
fi

# ---- Sanity check the binary ------------------------------------------------
if [ ! -x "$BIN" ]; then
    echo "AppRun: error: $BIN not found or not executable." >&2
    exit 127
fi

# ---- TTY fast path: just exec -----------------------------------------------
# A terminal-attached launch (running the AppImage directly from a shell)
# gets the raw experience: Ctrl+C works, exit codes propagate.
if [ -t 1 ] && [ -t 2 ]; then
    exec "$BIN" "$@"
fi

# ---- Detached launch: log to file -------------------------------------------
# File-manager / app-drawer launches have no terminal. Capture everything.
LOG_FILE=""
for try_path in \
    "${XDG_DATA_HOME:-$HOME/.local/share}/mini-browser/launch.log" \
    "/tmp/mini-browser-logs/$USER/launch.log" \
    "/tmp/mini-browser-logs/launch.log"; do
    try_dir=$(dirname "$try_path")
    if mkdir -p "$try_dir" 2>/dev/null; then
        LOG_FILE="$try_path"
        break
    fi
done

write_log_header() {
    [ -z "$LOG_FILE" ] && return 0
    {
        echo "============================================================"
        echo "Mini Browser AppRun launch"
        echo "Timestamp:    $(date -Iseconds 2>/dev/null || date)"
        echo "AppImage:     ${APPIMAGE:-<not set>}"
        echo "AppDir:       $SCRIPT_DIR"
        echo "Binary:       $BIN"
        echo "Args:         $*"
        echo "DISPLAY:                 ${DISPLAY:-<unset>}"
        echo "WAYLAND_DISPLAY:         ${WAYLAND_DISPLAY:-<unset>}"
        echo "XDG_RUNTIME_DIR:         ${XDG_RUNTIME_DIR:-<unset>}"
        echo "DBUS_SESSION_BUS_ADDRESS: ${DBUS_SESSION_BUS_ADDRESS:-<unset>}"
        echo "GDK_BACKEND:             ${GDK_BACKEND:-<unset>}"
        echo "LD_PRELOAD:              ${LD_PRELOAD:-<unset>}"
        echo "WEBKIT_DISABLE_DMABUF_RENDERER: ${WEBKIT_DISABLE_DMABUF_RENDERER:-<unset>}"
        echo "User:                    $(id 2>/dev/null || echo unknown)"
        echo "---- Helpers check ----"
        echo "Helpers source: $HELPERS_SRC  exists=$([ -d "$HELPERS_SRC" ] && echo yes || echo no)"
        echo "Helpers dest:   $HELPERS_DST  exists=$([ -d "$HELPERS_DST" ] && echo yes || echo no)"
        echo "---- ldd of binary ----"
        ldd "$BIN" 2>&1 || true
        echo "---- unshare availability ----"
        if command -v unshare >/dev/null 2>&1; then
            if unshare --user --map-root-user true 2>/dev/null; then
                echo "unshare --user: available"
            else
                echo "unshare --user: NOT available"
            fi
            if unshare --mount --propagation private true 2>/dev/null; then
                echo "unshare --mount: available (no extra privileges)"
            else
                echo "unshare --mount: NOT available (kernel restrictions)"
            fi
        else
            echo "unshare: command not found"
        fi
        echo "---- launch strategy ----"
    } >> "$LOG_FILE" 2>&1 || true
}

# Decide which launch strategy to use based on what's available.
# NOTE: test actual mount-ns capability, not just `command -v unshare`.
# On Arch/CachyOS hardened kernels `unshare --mount` returns `Operation not permitted`
# even though the binary exists — previous check caused `unshare-bind-mount`
# to be chosen and then fail on double-click (no TTY).
launch_strategy=""
if [ -d "$HELPERS_DST" ] && [ -x "$HELPERS_DST/WebKitNetworkProcess" ]; then
    launch_strategy="system-helpers"
elif [ -d "$HELPERS_SRC" ] && unshare --mount --propagation private true 2>/dev/null; then
    launch_strategy="unshare-bind-mount"
elif [ -n "${LD_PRELOAD:-}" ]; then
    launch_strategy="ld-preload-only"
else
    launch_strategy="raw"
fi

write_log_header

if [ -z "$LOG_FILE" ]; then
    # Logging unavailable everywhere; just try our best.
    case "$launch_strategy" in
        system-helpers|unshare-bind-mount|ld-preload-only|raw)
            exec "$BIN" "$@" ;;
    esac
    exit 127
fi

{
    echo "Strategy: $launch_strategy"
    echo "---- exec ----"
} >> "$LOG_FILE" 2>&1 || true

# ---- Launch -----------------------------------------------------------------
exit_code=0
case "$launch_strategy" in
    system-helpers)
        # Debian/Ubuntu: system has the helpers at the multiarch path.
        "$BIN" "$@" >> "$LOG_FILE" 2>&1
        exit_code=$?
        ;;
    unshare-bind-mount)
        # Bind-mount our bundled helpers where libwebkitgtk-6.0.so.4 expects them.
        # Inner sh argv: sh HELPERS_SRC HELPERS_DST BIN user-args...
        # $1, $2 are bind source/dest; $3 onwards is binary path + user args.
        mkdir -p "$HELPERS_DST" 2>/dev/null || true
        if ! unshare --mount --propagation private sh -c '
            mount --bind "$1" "$2" || exit 1
            shift 2
            exec "$@"
        ' sh "$HELPERS_SRC" "$HELPERS_DST" "$BIN" "$@" >> "$LOG_FILE" 2>&1; then
            exit_code=$?
            # Fallback: mount-ns failed at runtime (kernel restriction)
            # Use LD_PRELOAD shim instead — this is what Arch/CachyOS needs.
            {
                echo "unshare bind-mount failed (exit $exit_code); falling back to LD_PRELOAD shim" >> "$LOG_FILE" 2>&1 || true
            }
            if [ -n "${LD_PRELOAD:-}" ]; then
                "$BIN" "$@" >> "$LOG_FILE" 2>&1
                exit_code=$?
            fi
        else
            exit_code=0
        fi
        ;;
    ld-preload-only)
        {
            echo "unshare unavailable; using LD_PRELOAD shim only."
        } >> "$LOG_FILE" 2>&1 || true
        "$BIN" "$@" >> "$LOG_FILE" 2>&1
        exit_code=$?
        ;;
    raw)
        {
            echo "No helpers fix available — launching as-is (likely to crash)."
        } >> "$LOG_FILE" 2>&1 || true
        "$BIN" "$@" >> "$LOG_FILE" 2>&1
        exit_code=$?
        ;;
esac

{
    echo "---- exit code: $exit_code at $(date -Iseconds 2>/dev/null || date) ----"
} >> "$LOG_FILE" 2>&1 || true

if [ "$exit_code" -ne 0 ] && [ "$exit_code" -ne 130 ]; then
    # Try to notify the user via desktop notification.
    if command -v notify-send >/dev/null 2>&1; then
        notify-send --urgency=critical \
            "Mini Browser failed to launch" \
            "Exit code $exit_code. Log: $LOG_FILE" \
            2>/dev/null || true
    fi
fi

exit "$exit_code"