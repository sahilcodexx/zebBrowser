/*
 * webkit-helper-redirect.c — LD_PRELOAD shim that fixes AppImage launch on
 * non-Debian systems.
 *
 * The bundled libwebkitgtk-6.0.so.4 (built on Ubuntu 24.04) spawns its helper
 * processes (WebKitNetworkProcess, WebKitWebProcess, WebKitGPUProcess) from
 * the Debian multiarch path:
 *
 *   /usr/lib/x86_64-linux-gnu/webkitgtk-6.0/<helper>
 *
 * The AppImage bundles the .so itself but, by default, NOT those helpers
 * (linuxdeploy only copies files our binary links against, not arbitrary
 * libexec-style helpers). On Debian/Ubuntu the system already has them at the
 * multiarch path, so launching works. On Arch, Fedora, openSUSE, etc. the
 * path doesn't exist and the parent process aborts before any window opens:
 *
 *   ** ERROR **: Unable to spawn a new child process: Failed to spawn child
 *   process "/usr/lib/x86_64-linux-gnu/webkitgtk-6.0/WebKitNetworkProcess"
 *   (No such file or directory)
 *
 * This shim intercepts execve() in the main process. If the target path does
 * not exist on the host AND matches the webkitgtk-6.0 helper pattern AND a
 * copy exists in the AppDir's bundled helpers directory, redirect to that.
 * Otherwise pass through unchanged (so the system helpers are used when
 * present, and unrelated paths work normally).
 *
 * Built into the AppImage at packaging time:
 *   gcc -shared -fPIC -o AppDir/usr/lib/libwebkit-helper-redirect.so \
 *       packaging/webkit-helper-redirect.c -ldl
 *
 * Loaded by AppRun via:
 *   export LD_PRELOAD="$APPDIR/usr/lib/libwebkit-helper-redirect.so${LD_PRELOAD:+:$LD_PRELOAD}"
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <dlfcn.h>
#include <limits.h>
#include <errno.h>

typedef int (*execve_fn_t)(const char *pathname, char *const argv[], char *const envp[]);

static execve_fn_t real_execve = NULL;
static char appdir_helpers[PATH_MAX];

static void resolve_real_execve(void) {
    real_execve = (execve_fn_t)dlsym(RTLD_NEXT, "execve");
}

static void __attribute__((constructor)) init(void) {
    resolve_real_execve();
    const char *appdir = getenv("APPDIR");
    if (appdir && *appdir) {
        snprintf(appdir_helpers, sizeof(appdir_helpers),
                 "%s/usr/lib/x86_64-linux-gnu/webkitgtk-6.0", appdir);
    } else {
        appdir_helpers[0] = '\0';
    }
}

int execve(const char *pathname, char *const argv[], char *const envp[]) {
    if (!real_execve) {
        resolve_real_execve();
        if (!real_execve) {
            errno = ENOSYS;
            return -1;
        }
    }

    /* Fast path: if the host already has the helper at the requested path,
     * just exec it. This is the common case on Debian/Ubuntu hosts where
     * the system webkitgtk-6.0-4 package provides the helpers natively. */
    if (pathname && access(pathname, F_OK) == 0) {
        return real_execve(pathname, argv, envp);
    }

    /* Slow path: requested path is missing. If it looks like a WebKitGTK 6
     * helper and we bundled a copy in the AppDir, redirect to that. */
    if (pathname && appdir_helpers[0] != '\0'
        && strstr(pathname, "/webkitgtk-6.0/WebKit")) {
        const char *base = strrchr(pathname, '/');
        if (base) {
            char redirected[PATH_MAX];
            int n = snprintf(redirected, sizeof(redirected),
                             "%s%s", appdir_helpers, base);
            if (n > 0 && n < (int)sizeof(redirected)) {
                if (access(redirected, F_OK) == 0) {
                    /* Log to stderr so the user can see it in launch.log
                     * if AppRun captured stderr. Useful for diagnostics. */
                    fprintf(stderr,
                            "[webkit-helper-redirect] %s -> %s\n",
                            pathname, redirected);
                    return real_execve(redirected, argv, envp);
                }
            }
        }
    }

    return real_execve(pathname, argv, envp);
}