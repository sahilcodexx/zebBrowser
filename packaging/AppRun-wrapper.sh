#!/bin/sh
set -eu

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BIN="$SCRIPT_DIR/usr/bin/mini-browser"
HELPERS="$SCRIPT_DIR/w"
SHIM="$SCRIPT_DIR/usr/lib/libwebkit-helper-redirect.so"
SYSTEM_HELPERS=""

for helper_dir in \
    /usr/lib/webkitgtk-6.0 \
    /usr/lib/x86_64-linux-gnu/webkitgtk-6.0 \
    /usr/lib64/webkitgtk-6.0; do
    if [ -x "$helper_dir/WebKitNetworkProcess" ]; then
        SYSTEM_HELPERS="$helper_dir"
        break
    fi
done

cd "$SCRIPT_DIR"

if [ -z "${GDK_BACKEND:-}" ]; then
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
        GDK_BACKEND=wayland
    elif [ -n "${DISPLAY:-}" ]; then
        GDK_BACKEND=x11
    fi
    export GDK_BACKEND
fi

if [ "${WEBKIT_DISABLE_DMABUF_RENDERER+x}" = x ]; then
    export WEBKIT_DISABLE_DMABUF_RENDERER
fi
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=${WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS:-1}
export APPDIR="$SCRIPT_DIR"

if [ -d "$HELPERS" ]; then
    export WEBKIT_EXEC_PATH="$HELPERS"
    if [ -d "$HELPERS/injected-bundle" ]; then
        export WEBKIT_INJECTED_BUNDLE_PATH="$HELPERS/injected-bundle"
    fi
fi

if [ ! -f "$HELPERS/.relocated" ] && [ -z "$SYSTEM_HELPERS" ] && [ -f "$SHIM" ]; then
    export LD_PRELOAD="$SHIM${LD_PRELOAD:+:$LD_PRELOAD}"
fi

GIO_PATHS=""
for module_dir in \
    "$SCRIPT_DIR/usr/lib/gio/modules" \
    "$SCRIPT_DIR/usr/lib/x86_64-linux-gnu/gio/modules"; do
    if [ -d "$module_dir" ]; then
        if [ -n "$GIO_PATHS" ]; then
            GIO_PATHS="$GIO_PATHS:$module_dir"
        else
            GIO_PATHS="$module_dir"
        fi
    fi
done
if [ -n "$GIO_PATHS" ]; then
    export GIO_MODULE_DIR="$GIO_PATHS"
fi

if [ -d "$SCRIPT_DIR/usr/share/glib-2.0/schemas" ]; then
    export GSETTINGS_SCHEMA_DIR="$SCRIPT_DIR/usr/share/glib-2.0/schemas${GSETTINGS_SCHEMA_DIR:+:$GSETTINGS_SCHEMA_DIR}"
fi

if [ -d "$SCRIPT_DIR/usr/share" ]; then
    export XDG_DATA_DIRS="$SCRIPT_DIR/usr/share${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
fi

if [ -d "$SCRIPT_DIR/usr/lib/gstreamer-1.0" ]; then
    export GST_PLUGIN_SYSTEM_PATH_1_0="$SCRIPT_DIR/usr/lib/gstreamer-1.0"
    export GST_PLUGIN_PATH_1_0="$SCRIPT_DIR/usr/lib/gstreamer-1.0"
else
    for plugin_dir in \
        /usr/lib/x86_64-linux-gnu/gstreamer-1.0 \
        /usr/lib/gstreamer-1.0 \
        /usr/lib64/gstreamer-1.0; do
        if [ -d "$plugin_dir" ]; then
            export GST_PLUGIN_SYSTEM_PATH_1_0="$plugin_dir"
            export GST_PLUGIN_PATH_1_0="$plugin_dir"
            break
        fi
    done
fi

for pixbuf_cache in "$SCRIPT_DIR"/usr/lib/gdk-pixbuf-2.0/*/loaders.cache; do
    if [ -f "$pixbuf_cache" ]; then
        export GDK_PIXBUF_MODULE_FILE="$pixbuf_cache"
        break
    fi
done

if [ ! -x "$BIN" ]; then
    printf 'AppRun: error: %s not found or not executable.\n' "$BIN" >&2
    exit 127
fi

if [ -t 1 ] && [ -t 2 ]; then
    exec "$BIN" "$@"
fi

LOG_FILE=""
for try_path in \
    "${XDG_DATA_HOME:-${HOME:-/tmp}/.local/share}/mini-browser/launch.log" \
    "/tmp/mini-browser-logs/${USER:-unknown}/launch.log" \
    "/tmp/mini-browser-logs/launch.log"; do
    try_dir=$(dirname "$try_path")
    if mkdir -p "$try_dir" 2>/dev/null; then
        LOG_FILE="$try_path"
        break
    fi
done

if [ -z "$LOG_FILE" ]; then
    exec "$BIN" "$@"
fi

{
    printf 'Mini Browser AppRun launch\n'
    printf 'Timestamp: %s\n' "$(date -Iseconds 2>/dev/null || date)"
    printf 'AppDir: %s\n' "$SCRIPT_DIR"
    printf 'Binary: %s\n' "$BIN"
    printf 'Args: %s\n' "$*"
    printf 'GDK_BACKEND: %s\n' "${GDK_BACKEND:-<unset>}"
    printf 'WEBKIT_EXEC_PATH: %s\n' "${WEBKIT_EXEC_PATH:-<unset>}"
    printf 'WEBKIT_INJECTED_BUNDLE_PATH: %s\n' "${WEBKIT_INJECTED_BUNDLE_PATH:-<unset>}"
    printf 'WEBKIT_DISABLE_DMABUF_RENDERER: %s\n' "${WEBKIT_DISABLE_DMABUF_RENDERER:-<unset>}"
    printf 'LD_PRELOAD: %s\n' "${LD_PRELOAD:-<unset>}"
    printf 'GIO_MODULE_DIR: %s\n' "${GIO_MODULE_DIR:-<unset>}"
    printf 'GST_PLUGIN_SYSTEM_PATH_1_0: %s\n' "${GST_PLUGIN_SYSTEM_PATH_1_0:-<unset>}"
    printf 'XDG_DATA_DIRS: %s\n' "${XDG_DATA_DIRS:-<unset>}"
    printf 'System helpers: %s\n' "${SYSTEM_HELPERS:-<none>}"
    printf 'Relocated helpers: %s\n' "$([ -f "$HELPERS/.relocated" ] && printf yes || printf no)"
} >> "$LOG_FILE"

set +e
"$BIN" "$@" >> "$LOG_FILE" 2>&1
exit_code=$?
set -e
printf 'Exit code: %s\n' "$exit_code" >> "$LOG_FILE"
exit "$exit_code"
