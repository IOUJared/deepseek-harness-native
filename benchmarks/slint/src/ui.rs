slint::slint! {
    export struct ChatRow { index: int, role: string, body: string, footer: string }
    export struct RectGeometry { x: float, y: float, width: float, height: float }

    component FlatButton inherits Rectangle {
        in property <string> label;
        in property <bool> active: false;
        in property <bool> primary: false;
        callback clicked;
        background: primary ? #c4a7e7 : active || pointer.has-hover ? #26233a : #1f1d2e;
        border-radius: 6px;
        border-width: primary ? 0px : 1px;
        border-color: active ? #c4a7e7 : #403d52;
        Text { text: root.label; color: root.primary ? #191724 : #e0def4; vertical-alignment: center; horizontal-alignment: center; overflow: elide; }
        pointer := TouchArea { clicked => { root.clicked(); } }
    }

    export component BenchWindow inherits Window {
        title: "Harness — Slint toolkit comparison";
        preferred-width: 1200px;
        preferred-height: 800px;
        min-width: root.benchmark-mode ? 1200px : 880px;
        min-height: root.benchmark-mode ? 800px : 720px;
        max-width: root.benchmark-mode ? 1200px : 100000px;
        max-height: root.benchmark-mode ? 800px : 100000px;
        background: #191724;
        default-font-family: "DejaVu Sans";
        default-font-size: 14px;

        in property <[ChatRow]> visible-rows;
        in property <[string]> workspaces;
        in property <[string]> sessions;
        in property <int> selected-workspace: 0;
        in property <int> selected-session: 0;
        in property <int> total-rows: 1000;
        in-out property <float> scroll-offset: 0;
        in-out property <string> draft;
        in property <bool> benchmark-mode: false;
        out property <float> viewport-height: transcript.height / 1px;
        out property <float> logical-width: self.width / 1px;
        out property <float> logical-height: self.height / 1px;
        out property <RectGeometry> sidebar-geometry: { x: sidebar.x / 1px, y: sidebar.y / 1px, width: sidebar.width / 1px, height: sidebar.height / 1px };
        out property <RectGeometry> header-geometry: { x: header.absolute-position.x / 1px, y: header.absolute-position.y / 1px, width: header.width / 1px, height: header.height / 1px };
        out property <RectGeometry> transcript-geometry: { x: transcript.absolute-position.x / 1px, y: transcript.absolute-position.y / 1px, width: transcript.width / 1px, height: transcript.height / 1px };
        out property <RectGeometry> composer-geometry: { x: composer.absolute-position.x / 1px, y: composer.absolute-position.y / 1px, width: composer.width / 1px, height: composer.height / 1px };
        callback viewport-changed;
        callback scroll-request(float);
        callback select-workspace(int);
        callback select-session(int);
        callback send;
        callback stop;
        callback settings;
        changed scroll-offset => { root.viewport-changed(); }
        changed viewport-height => { root.viewport-changed(); }

        sidebar := Rectangle {
            x: 0px; y: 0px;
            width: 260px;
            height: parent.height;
            background: #1f1d2e;
            Text { x: 20px; y: 22px; width: 220px; height: 22px; text: "DeepSeek Harness"; color: #e0def4; font-weight: 600; }
            Text { x: 20px; y: 48px; width: 220px; height: 20px; text: "Native toolkit comparison"; color: #908caa; }
            Text { x: 20px; y: 90px; width: 220px; height: 20px; text: "WORKSPACES"; color: #6e6a86; }
            for workspace[i] in root.workspaces : FlatButton {
                x: 16px; y: 118px + i * 36px; width: 228px; height: 32px;
                label: workspace;
                active: i == root.selected-workspace;
                clicked => { root.select-workspace(i); }
            }
            Text { x: 20px; y: 238px; width: 220px; height: 20px; text: "SESSIONS"; color: #6e6a86; }
            for session[i] in root.sessions : FlatButton {
                x: 16px; y: 266px + i * 32px; width: 228px; height: 28px;
                label: session;
                active: i == root.selected-session;
                clicked => { root.select-session(i); }
            }
            FlatButton { x: 16px; y: parent.height - 48px; width: 228px; height: 32px; label: "Settings"; clicked => { root.settings(); } }
            Rectangle { x: parent.width - 1px; width: 1px; height: parent.height; background: #403d52; }
        }
        content := Rectangle {
            x: 260px; y: 0px;
            width: parent.width - self.x;
            height: parent.height;
            header := Rectangle {
                x: 0px; y: 0px;
                height: 48px;
                width: parent.width;
                background: #1f1d2e;
                Text { x: 16px; width: parent.width - 260px; height: 48px; text: root.workspaces[root.selected-workspace] + " / " + root.sessions[root.selected-session]; color: #e0def4; vertical-alignment: center; overflow: elide; }
                Text { x: parent.width - 244px; width: 228px; height: 48px; text: "Synthetic · " + root.total-rows + " rows"; color: #908caa; horizontal-alignment: right; vertical-alignment: center; }
                Rectangle { y: 47px; width: parent.width; height: 1px; background: #403d52; }
            }
            transcript := Rectangle {
                x: 0px; y: 48px;
                width: parent.width;
                height: parent.height - 48px - 104px;
                clip: true;
                background: #191724;
                for row in root.visible-rows : Rectangle {
                    x: 0;
                    y: row.index * 72px - root.scroll-offset * 1px;
                    width: parent.width - 10px;
                    height: 72px;
                    Text { x: 16px; y: 6px; width: parent.width - 32px; height: 20px; text: row.role; color: row.role == "You" ? #e0def4 : #c4a7e7; font-weight: 600; overflow: elide; }
                    Text { x: 16px; y: 26px; width: parent.width - 32px; height: 20px; text: row.body; color: #e0def4; overflow: elide; }
                    Text { x: 16px; y: 48px; width: parent.width - 32px; height: 20px; text: row.footer; color: #6e6a86; overflow: elide; }
                    Rectangle { y: 71px; width: parent.width; height: 1px; background: #403d52; }
                }
                TouchArea {
                    scroll-event(event) => {
                        root.scroll-request(-event.delta-y / 1px);
                        return accept;
                    }
                }
                Rectangle {
                    x: parent.width - 8px; width: 4px;
                    y: root.scroll-offset / max(1, root.total-rows * 72 - root.viewport-height) * max(0px, parent.height - self.height);
                    height: max(24px, parent.height * root.viewport-height / max(1, root.total-rows * 72));
                    background: #403d52; border-radius: 2px;
                }
            }
            composer := Rectangle {
                x: 0px; y: parent.height - 104px;
                height: 104px;
                width: parent.width;
                background: #1f1d2e;
                Rectangle { x: 0px; y: 0px; width: parent.width; height: 1px; background: #403d52; }
                input-box := Rectangle {
                    x: 16px; y: 16px; width: parent.width - 188px; height: 48px;
                    background: #191724; border-radius: 6px; border-width: 1px; border-color: input.has-focus ? #c4a7e7 : #403d52;
                    input := TextInput {
                        x: 12px; y: 12px; width: parent.width - 24px; height: 24px;
                        text <=> root.draft;
                        color: #e0def4;
                        selection-background-color: #403d52;
                        selection-foreground-color: #e0def4;
                        single-line: true;
                        enabled: !root.benchmark-mode;
                        read-only: root.benchmark-mode;
                        accepted => { root.send(); }
                    }
                    if root.draft == "" && !input.has-focus : Text { x: 12px; y: 12px; width: parent.width - 24px; height: 24px; text: "Write a synthetic message…"; color: #908caa; }
                }
                FlatButton { x: parent.width - 156px; y: 16px; width: 68px; height: 48px; label: "Send"; primary: true; clicked => { root.send(); } }
                FlatButton { x: parent.width - 80px; y: 16px; width: 64px; height: 48px; label: "Stop"; clicked => { root.stop(); } }
                Text { x: 16px; y: 72px; width: parent.width - 32px; height: 20px; text: "Synthetic only — no models, network, or saved state"; color: #6e6a86; overflow: elide; }
            }
        }
    }
}
