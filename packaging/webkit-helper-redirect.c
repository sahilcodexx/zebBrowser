/*
 * webkit-helper-redirect.c — LD_PRELOAD shim that fixes AppImage launch on
 * non-Debian systems.
 *
 * The bundled libwebkitgtk-6.0.so.4 spawns its helper processes from the
 * Debian multiarch path /usr/lib/x86_64-linux-gnu/webkitgtk-6.0/. On hosts
 * without that directory (Arch, Fedora, openSUSE, CachyOS, ...) the parent
 * process aborts before any window opens. This shim intercepts the spawn
 * paths and redirects them to the bundled copies when the system path is
 * missing.
 *
 * Intercepted entry points (we cast a wide net because GLib and glibc have
 * used different exec* / spawn* functions over the years and we don't know
 * in advance which one WebKitGTK 6 + the bundled glibc will end up using):
 *   execve, execv, execvp, execvpe, execl, execlp, execle, posix_spawn
 *
 * Set WEBKIT_HELPER_REDIRECT_DEBUG=1 in the environment to print every
 * intercepted call to stderr (handy for diagnosing future WebKitGTK changes).
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <dlfcn.h>
#include <limits.h>
#include <errno.h>
#include <spawn.h>

extern char **environ;

typedef int (*execve_fn_t)(const char *, char *const[], char *const[]);
typedef int (*execv_fn_t)(const char *, char *const[]);
typedef int (*execvp_fn_t)(const char *, char *const[]);
typedef int (*execvpe_fn_t)(const char *, char *const[], char *const[]);
typedef int (*execl_fn_t)(const char *, const char *, ...);
typedef int (*posix_spawn_fn_t)(pid_t *, const char *,
                                const posix_spawn_file_actions_t *,
                                const posix_spawnattr_t *,
                                char *const[], char *const[]);

static execve_fn_t    real_execve   = NULL;
static execv_fn_t     real_execv    = NULL;
static execvp_fn_t    real_execvp   = NULL;
static execvpe_fn_t   real_execvpe  = NULL;
static posix_spawn_fn_t real_posix_spawn = NULL;

static char appdir_helpers[PATH_MAX];
static int  debug_log = 0;

static void dbg(const char *fmt, ...) {
    if (!debug_log) return;
    va_list ap;
    va_start(ap, fmt);
    fputs("[webkit-helper-redirect] ", stderr);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    va_end(ap);
}

static void resolve_symbols(void) {
    if (!real_execve)
        real_execve  = (execve_fn_t)dlsym(RTLD_NEXT, "execve");
    if (!real_execv)
        real_execv   = (execv_fn_t) dlsym(RTLD_NEXT, "execv");
    if (!real_execvp)
        real_execvp  = (execvp_fn_t)dlsym(RTLD_NEXT, "execvp");
    if (!real_execvpe)
        real_execvpe = (execvpe_fn_t)dlsym(RTLD_NEXT, "execvpe");
    if (!real_posix_spawn)
        real_posix_spawn = (posix_spawn_fn_t)dlsym(RTLD_NEXT, "posix_spawn");
}

static const char *base_name(const char *path) {
    if (!path) return NULL;
    const char *slash = strrchr(path, '/');
    return slash ? slash : path;
}

/* Returns the redirected absolute path on success (caller must not free), or
 * NULL when no redirect should happen. */
static const char *maybe_redirect(const char *pathname) {
    if (!pathname || pathname[0] != '/') return NULL;
    if (access(pathname, X_OK) == 0)     return NULL;
    if (!appdir_helpers[0])             return NULL;
    /* Match the Debian-style multiarch helper location. We don't try to be
     * clever about matching the exact helper name — any absolute path
     * containing the marker gets the basename substituted. */
    if (!strstr(pathname, "/webkitgtk-6.0/")) return NULL;
    const char *base = base_name(pathname);
    if (!base || base[0] != '/') return NULL;
    static char redirected[PATH_MAX];
    int n = snprintf(redirected, sizeof(redirected), "%s%s", appdir_helpers, base);
    if (n <= 0 || n >= (int)sizeof(redirected)) return NULL;
    if (access(redirected, F_OK) != 0)          return NULL;
    return redirected;
}

static void __attribute__((constructor)) init(void) {
    resolve_symbols();
    const char *appdir = getenv("APPDIR");
    if (appdir && *appdir) {
        snprintf(appdir_helpers, sizeof(appdir_helpers),
                 "%s/w", appdir);
        if (access(appdir_helpers, F_OK) != 0) {
            snprintf(appdir_helpers, sizeof(appdir_helpers),
                     "%s/usr/lib/x86_64-linux-gnu/webkitgtk-6.0", appdir);
        }
    } else {
        appdir_helpers[0] = '\0';
    }
    debug_log = (getenv("WEBKIT_HELPER_REDIRECT_DEBUG") != NULL);
    dbg("shim loaded; APPDIR=%s helpers=%s real_execve=%p",
        appdir ? appdir : "<unset>", appdir_helpers,
        (void *)real_execve);
}

/* ---- execve ---- */
int execve(const char *pathname, char *const argv[], char *const envp[]) {
    if (!real_execve) resolve_symbols();
    dbg("execve(%s)", pathname ? pathname : "<null>");
    const char *r = maybe_redirect(pathname);
    if (r) {
        dbg("  -> redirecting to %s", r);
        return real_execve(r, argv, envp);
    }
    return real_execve(pathname, argv, envp);
}

/* ---- execv ---- */
int execv(const char *pathname, char *const argv[]) {
    if (!real_execv) resolve_symbols();
    dbg("execv(%s)", pathname ? pathname : "<null>");
    const char *r = maybe_redirect(pathname);
    if (r) {
        dbg("  -> redirecting to %s", r);
        return real_execv(r, argv);
    }
    return real_execv(pathname, argv);
}

/* ---- execvp ---- */
int execvp(const char *file, char *const argv[]) {
    if (!real_execvp) resolve_symbols();
    dbg("execvp(%s)", file ? file : "<null>");
    /* execvp searches PATH; for relative or bare names we don't redirect.
     * If file is absolute and matches, redirect. */
    if (file && file[0] == '/') {
        const char *r = maybe_redirect(file);
        if (r) {
            dbg("  -> redirecting to %s", r);
            return real_execve(r, argv, environ);
        }
    }
    return real_execvp(file, argv);
}

/* ---- execvpe (GNU extension) ---- */
int execvpe(const char *file, char *const argv[], char *const envp[]) {
    if (!real_execvpe) resolve_symbols();
    dbg("execvpe(%s)", file ? file : "<null>");
    if (file && file[0] == '/') {
        const char *r = maybe_redirect(file);
        if (r) {
            dbg("  -> redirecting to %s", r);
            return real_execve(r, argv, envp);
        }
    }
    return real_execvpe(file, argv, envp);
}

/* ---- posix_spawn ---- */
int posix_spawn(pid_t *pid, const char *path,
                const posix_spawn_file_actions_t *file_actions,
                const posix_spawnattr_t *attrp,
                char *const argv[], char *const envp[]) {
    if (!real_posix_spawn) resolve_symbols();
    dbg("posix_spawn(%s)", path ? path : "<null>");
    const char *r = maybe_redirect(path);
    if (r) {
        dbg("  -> redirecting to %s", r);
        return real_posix_spawn(pid, r, file_actions, attrp, argv, envp);
    }
    return real_posix_spawn(pid, path, file_actions, attrp, argv, envp);
}
