/* PUBLIC private-seat probe, libei 1.6 API from installed libei.h.
 * Sends only "public native keyboard check", never Enter. Not application/IME evidence.
 * Compile: cc -std=c11 -Wall -Wextra -Werror ... $(pkg-config --cflags --libs libei-1.0)
 * Caller must own daemon and pass its pinned pidfd. No ambient socket fallback.
 */
#define _GNU_SOURCE
#include <libei.h>
#include <linux/input-event-codes.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <poll.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct probe {
    struct ei *ei;
    struct ei_device *device;
    struct ei_ping *ping;
    int owner_fd;
    pid_t owner_pid;
    unsigned long long owner_start;
    uint64_t deadline;
    unsigned presses;
    uint32_t held;
    int button, ready, pong, failed, seats;
};

static unsigned long long number(const char *s) {
    char *end = NULL;
    errno = 0;
    unsigned long long n = strtoull(s, &end, 10);
    if (!*s || errno || *end) return 0;
    for (const char *digit = s; *digit; digit++) if (*digit < '0' || *digit > '9') return 0;
    return n;
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
    return field && number(field) == p->owner_start;
}

static int events(struct probe *p) {
    if (!owner_alive(p) || ei_now(p->ei) >= p->deadline) return 0;
    ei_dispatch(p->ei);
    struct ei_event *event;
    while ((event = ei_get_event(p->ei))) {
        enum ei_event_type type = ei_event_get_type(event);
        if (type == EI_EVENT_DISCONNECT) p->failed = 1;
        else if (type == EI_EVENT_SEAT_ADDED) {
            struct ei_seat *seat = ei_event_get_seat(event);
            if (++p->seats != 1 || !ei_seat_has_capability(seat, EI_DEVICE_CAP_KEYBOARD)) p->failed = 1;
            else ei_seat_bind_capabilities(seat, EI_DEVICE_CAP_KEYBOARD, EI_DEVICE_CAP_POINTER_ABSOLUTE, EI_DEVICE_CAP_BUTTON, NULL);
        } else if (type == EI_EVENT_DEVICE_ADDED) {
            struct ei_device *device = ei_event_get_device(event);
            if (ei_device_has_capability(device, EI_DEVICE_CAP_KEYBOARD)) {
                if (p->device || !ei_device_has_capability(device, EI_DEVICE_CAP_POINTER_ABSOLUTE) ||
                    !ei_device_has_capability(device, EI_DEVICE_CAP_BUTTON)) p->failed = 1;
                else p->device = ei_device_ref(device);
            }
        } else if (type == EI_EVENT_DEVICE_RESUMED) {
            if (ei_event_get_device(event) == p->device) p->ready = 1;
        } else if (type == EI_EVENT_DEVICE_PAUSED || type == EI_EVENT_DEVICE_REMOVED) {
            if (ei_event_get_device(event) == p->device) { p->ready = 0; p->failed = 1; }
        } else if (type == EI_EVENT_SEAT_REMOVED) p->failed = 1;
        else if (type == EI_EVENT_PONG && p->ping && ei_event_pong_get_ping(event) == p->ping) p->pong = 1;
        ei_event_unref(event);
    }
    return !p->failed;
}

static int wait_events(struct probe *p, int milliseconds) {
    if (!events(p)) return 0;
    struct pollfd fds[] = {
        { .fd = ei_get_fd(p->ei), .events = POLLIN },
        { .fd = p->owner_fd, .events = POLLIN },
    };
    int result = poll(fds, 2, milliseconds);
    if (result < 0 && errno != EINTR) return 0;
    return events(p);
}

static int key(struct probe *p, uint32_t code, int down) {
    if (!p->ready || !events(p)) return 0;
    ei_device_keyboard_key(p->device, code, down);
    ei_device_frame(p->device, ei_now(p->ei));
    p->held = down ? code : 0;
    p->presses += down != 0;
    return wait_events(p, 12);
}

static int connect_owned(const char *root, const char *path, struct probe *p) {
    char resolved[PATH_MAX], runtime[PATH_MAX];
    struct stat st;
    if (!realpath(root, resolved) || strcmp(root, resolved) || lstat(root, &st) ||
        !S_ISDIR(st.st_mode) || st.st_uid != getuid() || (st.st_mode & 0777) != 0700) return -1;
    if (snprintf(runtime, sizeof(runtime), "%s/runtime", root) >= (int)sizeof(runtime)) return -1;
    if (!realpath(runtime, resolved) || strcmp(runtime, resolved) || lstat(runtime, &st) ||
        !S_ISDIR(st.st_mode) || st.st_uid != getuid() || (st.st_mode & 0777) != 0700) return -1;
    size_t prefix = strlen(runtime);
    if (strncmp(path, runtime, prefix) || path[prefix] != '/' ||
        strncmp(path + prefix + 1, "gamescope-", 10)) return -1;
    const char *name = path + prefix + 11;
    if (*name < '0' || *name > '9') return -1;
    while (*name >= '0' && *name <= '9') name++;
    if (strcmp(name, "-ei") || lstat(path, &st) || !S_ISSOCK(st.st_mode) || st.st_uid != getuid()) return -1;
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

int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--help")) {
        puts("native-seat-keyboard ABS_PRIVATE_ROOT ABS_EIS_SOCKET OWNER_PID OWNER_START_TICKS INHERITED_OWNER_PIDFD X Y WIDTH HEIGHT\n"
             "Fixed public native keyboard check only; no Enter, no ambient socket. Transport result is not App success.");
        return 0;
    }
    struct probe p = {0};
    const char *error = "invalid-explicit-arguments";
    int result = 1, fd = -1, started = 0;
    if (argc != 10 || argv[1][0] != '/' || argv[2][0] != '/') goto done;
    unsigned long long pid = number(argv[3]), start = number(argv[4]), inherited = number(argv[5]);
    unsigned long long x = number(argv[6]), y = number(argv[7]), width = number(argv[8]), height = number(argv[9]);
    if (!pid || pid > INT_MAX || !start || inherited < 3 || inherited > INT_MAX ||
        !x || !y || width < 200 || height < 200 || width > 4096 || height > 4096 || x >= width || y >= height) goto done;
    p.owner_pid = (pid_t)pid; p.owner_start = start; p.owner_fd = (int)inherited;
    /* Validate that the passed descriptor really is the supplied process's pidfd. */
    char info_path[64], line[128];
    snprintf(info_path, sizeof(info_path), "/proc/self/fdinfo/%d", p.owner_fd);
    FILE *info = fopen(info_path, "re");
    if (!info) goto done;
    int matching = 0;
    while (fgets(line, sizeof(line), info)) {
        int recorded = 0;
        if (sscanf(line, "Pid:\t%d", &recorded) == 1 && recorded == p.owner_pid) matching = 1;
    }
    fclose(info);
    error = "owned-socket-or-pidfd-refused";
    if (!matching || (fd = connect_owned(argv[1], argv[2], &p)) < 0) goto done;
    p.ei = ei_new_sender(NULL);
    if (!p.ei) { error = "ei-context-unavailable"; goto done; }
    ei_configure_name(p.ei, "PUBLIC native private-seat keyboard probe");
    int connected_fd = fd;
    fd = -1; /* ei_setup_backend_fd takes ownership; no second pathname lookup/default. */
    if (ei_setup_backend_fd(p.ei, connected_fd)) { error = "ei-backend-refused"; goto done; }
    p.deadline = ei_now(p.ei) + 8000000;
    error = "keyboard-resume-unavailable";
    while (!p.ready) if (!wait_events(&p, 20)) goto done;
    ei_device_start_emulating(p.device, 1); started = 1;
    error = "owned-pointer-click-interrupted";
    ei_device_pointer_motion_absolute(p.device, (double)x, (double)y);
    ei_device_frame(p.device, ei_now(p.ei));
    if (!wait_events(&p, 30) || !p.ready) goto done;
    ei_device_button_button(p.device, BTN_LEFT, 1); p.button = 1;
    ei_device_frame(p.device, ei_now(p.ei));
    if (!wait_events(&p, 30) || !p.ready) goto done;
    ei_device_button_button(p.device, BTN_LEFT, 0); p.button = 0;
    ei_device_frame(p.device, ei_now(p.ei));
    if (!wait_events(&p, 30) || !p.ready) goto done;
    static const uint32_t codes[] = {
        KEY_P, KEY_U, KEY_B, KEY_L, KEY_I, KEY_C, KEY_SPACE,
        KEY_N, KEY_A, KEY_T, KEY_I, KEY_V, KEY_E, KEY_SPACE,
        KEY_K, KEY_E, KEY_Y, KEY_B, KEY_O, KEY_A, KEY_R, KEY_D,
        KEY_SPACE, KEY_C, KEY_H, KEY_E, KEY_C, KEY_K,
    };
    error = "keyboard-sequence-interrupted";
    for (size_t i = 0; i < sizeof(codes) / sizeof(codes[0]); i++) {
        if (!key(&p, codes[i], 1) || !key(&p, codes[i], 0)) goto done;
    }
    ei_device_stop_emulating(p.device); started = 0;
    p.ping = ei_new_ping(p.ei);
    if (!p.ping) { error = "roundtrip-unavailable"; goto done; }
    ei_ping(p.ping);
    error = "roundtrip-interrupted";
    while (!p.pong) if (!wait_events(&p, 20)) goto done;
    if (!events(&p)) goto done; /* A synthetic PONG before disconnect is not success. */
    result = 0; error = "none";
done:
    if (started && p.ready && !p.failed && owner_alive(&p)) {
        if (p.held) ei_device_keyboard_key(p.device, p.held, 0);
        if (p.button) ei_device_button_button(p.device, BTN_LEFT, 0);
        ei_device_frame(p.device, ei_now(p.ei));
        ei_device_stop_emulating(p.device);
        ei_dispatch(p.ei);
    }
    if (p.ping) ei_ping_unref(p.ping);
    if (p.device) ei_device_unref(p.device);
    if (p.ei) ei_unref(p.ei);
    if (fd >= 0) close(fd);
    printf("{\"version\":1,\"publicOnly\":true,\"status\":\"%s\",\"errorCode\":\"%s\","
           "\"sequence\":\"public native keyboard check\",\"keyPressesSent\":%u,\"enterSent\":false,"
           "\"applicationInputQualified\":false,\"physicalImeQualified\":false}\n",
           result ? "failed" : "transport-roundtrip", error, p.presses);
    return result;
}
