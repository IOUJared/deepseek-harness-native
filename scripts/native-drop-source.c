/* PUBLIC native GTK3 drag source. No file content reads, uploads or App shortcuts.
 * Compile with pkg-config --cflags --libs gtk+-3.0.
 * Only text/uri-list and COPY are offered; never handles drag-data-delete.
 */
#define _POSIX_C_SOURCE 200809L
#include <gtk/gtk.h>
#include <gdk/gdkwayland.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

struct source {
    gchar *uris[3];
    guint files, begins, requests, ends;
    gboolean timed_out;
    GtkWidget *window;
};

static gboolean public_path(const char *path) {
    if (!path || path[0] != '/' || strlen(path) > 4096 || !g_utf8_validate(path, -1, NULL)) return FALSE;
    const char *leaf = strrchr(path, '/');
    if (!leaf || !leaf[1] || !strstr(leaf + 1, "PUBLIC")) return FALSE;
    for (const char *p = path; *p; p++) {
        if ((unsigned char)*p < 32 || (unsigned char)*p == 127) return FALSE;
        if (*p == '/' && (p[1] == '/' || (p[1] == '.' && (!p[2] || p[2] == '/' ||
            (p[2] == '.' && (!p[3] || p[3] == '/')))))) return FALSE;
    }
    return TRUE;
}

static void begin(GtkWidget *widget, GdkDragContext *context, gpointer data) {
    (void)widget; (void)context;
    struct source *s = data;
    s->begins++;
    printf("{\"version\":1,\"publicOnly\":true,\"phase\":\"drag-begin\",\"count\":%u,\"files\":%u,\"copyOnly\":true}\n", s->begins, s->files);
    fflush(stdout);
}

static void get_data(GtkWidget *widget, GdkDragContext *context, GtkSelectionData *selection,
                     guint info, guint time, gpointer data) {
    (void)widget; (void)context; (void)time;
    struct source *s = data;
    if (info != 1 || gtk_selection_data_get_target(selection) != gdk_atom_intern_static_string("text/uri-list")) return;
    gboolean supplied = gtk_selection_data_set_uris(selection, s->uris);
    s->requests++;
    printf("{\"version\":1,\"publicOnly\":true,\"phase\":\"drag-data-get\",\"count\":%u,\"files\":%u,\"uriListSupplied\":%s}\n", s->requests, s->files, supplied ? "true" : "false");
    fflush(stdout);
}

static void end(GtkWidget *widget, GdkDragContext *context, gpointer data) {
    (void)widget;
    struct source *s = data;
    s->ends++;
    printf("{\"version\":1,\"publicOnly\":true,\"phase\":\"drag-end\",\"count\":%u,\"selectedCopy\":%s,\"applicationDropQualified\":false}\n", s->ends,
           gdk_drag_context_get_selected_action(context) == GDK_ACTION_COPY ? "true" : "false");
    fflush(stdout);
}

static void mapped(GtkWidget *widget, gpointer data) {
    struct source *s = data;
    GtkAllocation allocation;
    gtk_widget_get_allocation(widget, &allocation);
    printf("{\"version\":1,\"publicOnly\":true,\"phase\":\"source-ready\",\"pid\":%ld,\"nativeWayland\":true,\"files\":%u,\"width\":%d,\"height\":%d,\"copyOnly\":true}\n",
           (long)getpid(), s->files, allocation.width, allocation.height);
    fflush(stdout);
}

static void destroyed(GtkWidget *widget, gpointer data) {
    (void)widget; (void)data;
    gtk_main_quit();
}

static gboolean timeout_source(gpointer data) {
    struct source *s = data;
    s->timed_out = TRUE;
    puts("{\"version\":1,\"publicOnly\":true,\"phase\":\"source-timeout\",\"applicationDropQualified\":false}");
    fflush(stdout);
    gtk_widget_destroy(s->window);
    return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--help")) {
        puts("native-drop-source --single ABS_PUBLIC_FILE | --multiple ABS_PUBLIC_FILE ABS_PUBLIC_FILE\n"
             "Native GTK3 whole-window COPY-only text/uri-list source; 45-second deadline, no file reads.");
        return 0;
    }
    struct source s = {0};
    if (argc == 3 && !strcmp(argv[1], "--single")) s.files = 1;
    else if (argc == 4 && !strcmp(argv[1], "--multiple")) s.files = 2;
    else { fputs("invalid-public-source-arguments\n", stderr); return 1; }
    for (guint i = 0; i < s.files; i++) {
        if (!public_path(argv[i + 2])) { fputs("absolute-PUBLIC-file-leaf-required\n", stderr); return 1; }
        s.uris[i] = g_filename_to_uri(argv[i + 2], NULL, NULL);
        if (!s.uris[i]) { fputs("public-uri-conversion-failed\n", stderr); return 1; }
    }
    /* Do not allow GTK to discover an X11 fallback, even if the caller makes a mistake. */
    gdk_set_allowed_backends("wayland");
    if (!gtk_init_check(NULL, NULL) || !GDK_IS_WAYLAND_DISPLAY(gdk_display_get_default())) {
        fputs("native-wayland-display-required\n", stderr);
        for (guint i = 0; i < s.files; i++) g_free(s.uris[i]);
        return 1;
    }
    s.window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(s.window), "PUBLIC native COPY drag source");
    gtk_window_set_default_size(GTK_WINDOW(s.window), 420, 540);
    GtkWidget *area = gtk_event_box_new();
    GtkWidget *label = gtk_label_new(s.files == 1 ? "PUBLIC one-file COPY source\nDrag anywhere in this window" :
                                                    "PUBLIC two-file COPY source\nDrag anywhere in this window");
    gtk_container_add(GTK_CONTAINER(area), label);
    gtk_container_add(GTK_CONTAINER(s.window), area);
    GtkTargetEntry target = { (gchar *)"text/uri-list", 0, 1 };
    gtk_drag_source_set(area, GDK_BUTTON1_MASK, &target, 1, GDK_ACTION_COPY);
    g_signal_connect(area, "drag-begin", G_CALLBACK(begin), &s);
    g_signal_connect(area, "drag-data-get", G_CALLBACK(get_data), &s);
    g_signal_connect(area, "drag-end", G_CALLBACK(end), &s);
    g_signal_connect(area, "map", G_CALLBACK(mapped), &s);
    g_signal_connect(s.window, "destroy", G_CALLBACK(destroyed), &s);
    guint timer = g_timeout_add_seconds(45, timeout_source, &s);
    gtk_widget_show_all(s.window);
    gtk_main();
    if (!s.timed_out) g_source_remove(timer);
    for (guint i = 0; i < s.files; i++) g_free(s.uris[i]);
    return s.timed_out ? 2 : 0;
}
