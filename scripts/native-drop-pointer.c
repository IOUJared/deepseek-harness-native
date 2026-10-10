/* PUBLIC private-compositor pointer transport, not App success evidence.
 * Compile against generated wlr-virtual-pointer-v1 header/code + wayland-client.
 * Requires a pinned owner pidfd and private socket; no ambient connection fallback.
 */
#define _GNU_SOURCE
#include <wayland-client.h>
#include "wlr-virtual-pointer-client.h"
#include <linux/input-event-codes.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <poll.h>
#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t interrupted;
struct probe {
    struct wl_display *display;
    struct wl_registry *registry;
    struct zwlr_virtual_pointer_manager_v1 *manager;
    struct zwlr_virtual_pointer_v1 *pointer;
    pid_t owner_pid;
    unsigned long long owner_start;
    int owner_fd, held, failed;
    uint32_t manager_id;
    unsigned managers, motions;
    uint64_t deadline;
};

static void interrupt(int sig) { (void)sig; interrupted = 1; }

static uint64_t now_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return 0;
    return (uint64_t)t.tv_sec * 1000 + (uint64_t)t.tv_nsec / 1000000;
}

static int number(const char *s, unsigned long long *result) {
    if (!s || !*s) return 0;
    for (const char *p = s; *p; p++) if (*p < '0' || *p > '9') return 0;
    char *end = NULL;
    errno = 0;
    *result = strtoull(s, &end, 10);
    return !errno && end && !*end;
}

static int owner_alive(struct probe *p) {
    struct pollfd fd = { .fd = p->owner_fd, .events = POLLIN };
    if (poll(&fd, 1, 0) != 0) return 0;
    char path[64], raw[4096];
    snprintf(path, sizeof(path), "/proc/%d/stat", p->owner_pid);
    FILE *stream = fopen(path, "re");
    if (!stream) return 0;
    size_t n = fread(raw, 1, sizeof(raw) - 1, stream);
    fclose(stream);
    raw[n] = 0;
    char *tail = strrchr(raw, ')');
    if (!tail || tail[1] != ' ') return 0;
    char *save = NULL, *field = strtok_r(tail + 2, " ", &save);
    for (int i = 3; field && i < 22; i++) field = strtok_r(NULL, " ", &save);
    unsigned long long start;
    return field && number(field, &start) && start == p->owner_start;
}

static int matching_pidfd(struct probe *p) {
    char path[64], line[128];
    snprintf(path, sizeof(path), "/proc/self/fdinfo/%d", p->owner_fd);
    FILE *info = fopen(path, "re");
    if (!info) return 0;
    int match = 0;
    while (fgets(line, sizeof(line), info)) {
        int pid = 0;
        if (sscanf(line, "Pid:\t%d", &pid) == 1 && pid == p->owner_pid) match = 1;
    }
    fclose(info);
    return match && owner_alive(p);
}

static int private_directory(const char *path) {
    char resolved[PATH_MAX];
    struct stat st;
    return realpath(path, resolved) && !strcmp(path, resolved) && !lstat(path, &st) &&
           S_ISDIR(st.st_mode) && st.st_uid == getuid() && (st.st_mode & 0777) == 0700;
}

static int connect_owned(const char *root, const char *path, struct probe *p) {
    char runtime[PATH_MAX], resolved[PATH_MAX];
    struct stat st;
    if (!private_directory(root) || snprintf(runtime, sizeof(runtime), "%s/runtime", root) >= (int)sizeof(runtime) ||
        !private_directory(runtime)) return -1;
    size_t prefix = strlen(runtime);
    if (strncmp(path, runtime, prefix) || path[prefix] != '/' ||
        strncmp(path + prefix + 1, "wayland-", 8)) return -1;
    const char *name = path + prefix + 9;
    unsigned long long slot;
    if (!number(name, &slot) || slot < 1 || slot > 32 || !realpath(path, resolved) || strcmp(path, resolved) ||
        lstat(path, &st) || !S_ISSOCK(st.st_mode) || st.st_uid != getuid()) return -1;
    struct sockaddr_un address = { .sun_family = AF_UNIX };
    if (strlen(path) >= sizeof(address.sun_path)) return -1;
    strcpy(address.sun_path, path);
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;
    struct ucred peer;
    socklen_t length = sizeof(peer);
    if (!owner_alive(p) || connect(fd, (struct sockaddr *)&address, sizeof(address)) ||
        getsockopt(fd, SOL_SOCKET, SO_PEERCRED, &peer, &length) || length != sizeof(peer) ||
        peer.uid != getuid() || peer.pid != p->owner_pid || !owner_alive(p)) {
        close(fd); return -1;
    }
    return fd;
}

/* Bounded Wayland dispatch: no blocking wl_display_roundtrip or global input. */
static int pump(struct probe *p, int milliseconds) {
    uint64_t now = now_ms();
    if (interrupted || p->failed || !now || now >= p->deadline || !owner_alive(p) || wl_display_get_error(p->display)) return 0;
    if (wl_display_dispatch_pending(p->display) < 0) return 0;
    while (wl_display_prepare_read(p->display) != 0) {
        if (interrupted || p->failed || now_ms() >= p->deadline || !owner_alive(p) ||
            wl_display_dispatch_pending(p->display) < 0) return 0;
    }
    int flushed = wl_display_flush(p->display);
    if (flushed < 0 && errno != EAGAIN) { wl_display_cancel_read(p->display); return 0; }
    now = now_ms();
    int budget = now < p->deadline ? (int)(p->deadline - now) : 0;
    if (milliseconds > budget) milliseconds = budget;
    struct pollfd fds[] = {
        { .fd = wl_display_get_fd(p->display), .events = POLLIN | (flushed < 0 ? POLLOUT : 0) },
        { .fd = p->owner_fd, .events = POLLIN },
    };
    int result = poll(fds, 2, milliseconds);
    if (result < 0 || fds[1].revents || (fds[0].revents & (POLLERR | POLLHUP | POLLNVAL))) {
        wl_display_cancel_read(p->display); return 0;
    }
    if (fds[0].revents & POLLIN) {
        if (wl_display_read_events(p->display) < 0) return 0;
    } else wl_display_cancel_read(p->display);
    if (wl_display_dispatch_pending(p->display) < 0) return 0;
    return !interrupted && !p->failed && !wl_display_get_error(p->display) && now_ms() < p->deadline && owner_alive(p);
}

struct sync_state { struct wl_callback *callback; int done; };
static void sync_done(void *data, struct wl_callback *callback, uint32_t serial) {
    (void)serial;
    struct sync_state *s = data;
    s->done = 1;
    wl_callback_destroy(callback);
    s->callback = NULL;
}
static const struct wl_callback_listener sync_listener = { .done = sync_done };

static int roundtrip(struct probe *p) {
    struct sync_state s = { .callback = wl_display_sync(p->display) };
    if (!s.callback) return 0;
    if (wl_callback_add_listener(s.callback, &sync_listener, &s)) {
        wl_callback_destroy(s.callback); return 0;
    }
    int ok = 1;
    while (!s.done) if (!pump(p, 20)) { ok = 0; break; }
    if (s.callback) wl_callback_destroy(s.callback);
    return ok && s.done && !p->failed && owner_alive(p);
}

static int settle(struct probe *p, unsigned milliseconds) {
    uint64_t until = now_ms() + milliseconds;
    while (now_ms() < until) if (!pump(p, 20)) return 0;
    return 1;
}

static void global(void *data, struct wl_registry *registry, uint32_t id, const char *name, uint32_t version) {
    struct probe *p = data;
    if (strcmp(name, "zwlr_virtual_pointer_manager_v1")) return;
    if (++p->managers != 1 || version < 1) { p->failed = 1; return; }
    p->manager_id = id;
    p->manager = wl_registry_bind(registry, id, &zwlr_virtual_pointer_manager_v1_interface, version < 2 ? version : 2);
    if (!p->manager) p->failed = 1;
}
static void global_remove(void *data, struct wl_registry *registry, uint32_t id) {
    (void)registry;
    struct probe *p = data;
    if (id == p->manager_id) p->failed = 1;
}
static const struct wl_registry_listener registry_listener = { .global = global, .global_remove = global_remove };

static int move(struct probe *p, uint32_t x, uint32_t y, uint32_t width, uint32_t height) {
    if (interrupted || !owner_alive(p) || now_ms() >= p->deadline) return 0;
    zwlr_virtual_pointer_v1_motion_absolute(p->pointer, (uint32_t)now_ms(), x, y, width, height);
    zwlr_virtual_pointer_v1_frame(p->pointer);
    p->motions++;
    return pump(p, 20);
}

int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--help")) {
        puts("native-drop-pointer ABS_PRIVATE_ROOT ABS_PRIVATE_WAYLAND_SOCKET OWNER_PID OWNER_START_TICKS INHERITED_OWNER_PIDFD SOURCE_X SOURCE_Y TARGET_X TARGET_Y WIDTH HEIGHT\n"
             "ROOT/runtime/wayland-N only; all coordinates inside <=4096 bounds. 8-second transport deadline. App success is separate.");
        return 0;
    }
    struct probe p = {0};
    unsigned long long values[9];
    int result = 1, fd = -1;
    const char *error = "invalid-explicit-arguments";
    if (argc != 12 || argv[1][0] != '/' || argv[2][0] != '/') goto done;
    for (int i = 0; i < 9; i++) if (!number(argv[i + 3], &values[i])) goto done;
    unsigned long long pid = values[0], start = values[1], inherited = values[2];
    uint64_t sx = values[3], sy = values[4], tx = values[5], ty = values[6], width = values[7], height = values[8];
    if (!pid || pid > INT_MAX || !start || inherited < 3 || inherited > INT_MAX ||
        width < 200 || height < 200 || width > 4096 || height > 4096 || sx >= width || tx >= width || sy >= height || ty >= height ||
        (llabs((long long)sx - (long long)tx) < 24 && llabs((long long)sy - (long long)ty) < 24)) goto done;
    p.owner_pid = (pid_t)pid; p.owner_start = start; p.owner_fd = (int)inherited;
    error = "owned-socket-or-pidfd-refused";
    if (!matching_pidfd(&p) || (fd = connect_owned(argv[1], argv[2], &p)) < 0) goto done;
    struct sigaction action = { .sa_handler = interrupt };
    sigemptyset(&action.sa_mask);
    sigaction(SIGINT, &action, NULL); sigaction(SIGTERM, &action, NULL);
    signal(SIGPIPE, SIG_IGN);
    uint64_t now = now_ms();
    if (!now) { error = "monotonic-clock-unavailable"; goto done; }
    p.deadline = now + 8000;
    /* Ownership transfers even on connection initialization failure. */
    int connected = fd;
    fd = -1;
    p.display = wl_display_connect_to_fd(connected);
    if (!p.display) { error = "private-wayland-connection-failed"; goto done; }
    p.registry = wl_display_get_registry(p.display);
    error = "virtual-pointer-unavailable";
    if (!p.registry || wl_registry_add_listener(p.registry, &registry_listener, &p) ||
        !roundtrip(&p) || !p.manager || p.managers != 1) goto done;
    p.pointer = zwlr_virtual_pointer_manager_v1_create_virtual_pointer(p.manager, NULL);
    if (!p.pointer || !roundtrip(&p)) goto done;
    error = "private-drag-interrupted";
    if (!move(&p, (uint32_t)sx, (uint32_t)sy, (uint32_t)width, (uint32_t)height) || !roundtrip(&p) || !settle(&p, 160)) goto done;
    zwlr_virtual_pointer_v1_button(p.pointer, (uint32_t)now_ms(), BTN_LEFT, WL_POINTER_BUTTON_STATE_PRESSED);
    p.held = 1;
    zwlr_virtual_pointer_v1_frame(p.pointer);
    if (!roundtrip(&p) || !settle(&p, 80)) goto done;
    for (int i = 1; i <= 40; i++) {
        uint32_t x = (uint32_t)((long long)sx + ((long long)tx - (long long)sx) * i / 40);
        uint32_t y = (uint32_t)((long long)sy + ((long long)ty - (long long)sy) * i / 40);
        if (!move(&p, x, y, (uint32_t)width, (uint32_t)height) || !settle(&p, 20)) goto done;
    }
    if (!roundtrip(&p) || !settle(&p, 180)) goto done;
    zwlr_virtual_pointer_v1_button(p.pointer, (uint32_t)now_ms(), BTN_LEFT, WL_POINTER_BUTTON_STATE_RELEASED);
    p.held = 0;
    zwlr_virtual_pointer_v1_frame(p.pointer);
    error = "private-roundtrip-interrupted";
    if (!roundtrip(&p) || !settle(&p, 160) || !roundtrip(&p)) goto done;
    result = 0; error = "none";
done:
    if (p.held && p.pointer && p.display && !wl_display_get_error(p.display) && owner_alive(&p)) {
        zwlr_virtual_pointer_v1_button(p.pointer, (uint32_t)now_ms(), BTN_LEFT, WL_POINTER_BUTTON_STATE_RELEASED);
        zwlr_virtual_pointer_v1_frame(p.pointer);
        /* Best effort, nonblocking even on timeout/interruption; never reconnect. */
        wl_display_flush(p.display);
        p.held = 0;
    }
    if (p.pointer) zwlr_virtual_pointer_v1_destroy(p.pointer);
    if (p.manager) zwlr_virtual_pointer_manager_v1_destroy(p.manager);
    if (p.registry) wl_registry_destroy(p.registry);
    if (p.display) {
        if (owner_alive(&p) && !wl_display_get_error(p.display)) wl_display_flush(p.display);
        wl_display_disconnect(p.display);
    }
    if (fd >= 0) close(fd);
    printf("{\"version\":1,\"publicOnly\":true,\"status\":\"%s\",\"errorCode\":\"%s\",\"motionsSent\":%u,\"applicationDropQualified\":false,\"physicalPointerQualified\":false}\n",
           result ? "failed" : "transport-roundtrip", error, p.motions);
    return result;
}
