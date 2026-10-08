//! PUBLIC actual-App native scroll/reflow fixture. No Host, worker or effect executor.
//! Opt in with public-layout-fixture; parent owns serial builds and Wayland runs.
//! Timers cover deadline, post-observation sampling hold and final paint grace only.
#![allow(dead_code)]
#[path = "../src/codex.rs"]
mod codex;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/design.rs"]
mod design;
#[path = "../src/export_file.rs"]
mod export_file;
#[path = "../src/exporter.rs"]
mod exporter;
#[path = "../src/interactions.rs"]
mod interactions;
#[path = "../src/known.rs"]
mod known;
#[path = "../src/management.rs"]
mod management;
#[path = "../src/plugins.rs"]
mod plugins;
#[path = "../src/reducer.rs"]
mod reducer;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/smoke.rs"]
mod smoke;
#[path = "../src/ui.rs"]
mod ui;
#[path = "../src/worker.rs"]
mod worker;

#[cfg(feature = "public-layout-fixture")]
mod driver {
    use super::ui;
    use iced::advanced as core;
    use iced::advanced::widget::{
        Id, Operation,
        operation::{Outcome, Scrollable},
    };
    use iced::{Element, Rectangle, Size, Subscription, Task, Vector, window};
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        fs,
        io::Write,
        os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        path::{Component, Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    type ShieldCounts = Arc<[AtomicU64; 4]>;
    fn capture_input<M>(
        event: &iced::Event,
        counts: &ShieldCounts,
        shell: &mut core::Shell<'_, M>,
    ) -> bool {
        let category = match event {
            iced::Event::Mouse(_) => 0,
            iced::Event::Keyboard(_) => 1,
            iced::Event::Touch(_) => 2,
            iced::Event::InputMethod(_) => 3,
            _ => return false,
        };
        counts[category].fetch_add(1, Ordering::Relaxed);
        shell.capture_event();
        true
    }
    // Transparent fixture-only wrapper: production layout, drawing and operation
    // traversal are unchanged. Physical events never reach the actual App widgets.
    struct InputShield<'a> {
        content: Element<'a, Message>,
        counts: ShieldCounts,
    }
    impl core::Widget<Message, iced::Theme, iced::Renderer> for InputShield<'_> {
        fn tag(&self) -> core::widget::tree::Tag {
            self.content.as_widget().tag()
        }
        fn state(&self) -> core::widget::tree::State {
            self.content.as_widget().state()
        }
        fn children(&self) -> Vec<core::widget::Tree> {
            self.content.as_widget().children()
        }
        fn diff(&self, tree: &mut core::widget::Tree) {
            self.content.as_widget().diff(tree);
        }
        fn size(&self) -> Size<iced::Length> {
            self.content.as_widget().size()
        }
        fn size_hint(&self) -> Size<iced::Length> {
            self.content.as_widget().size_hint()
        }
        fn layout(
            &mut self,
            tree: &mut core::widget::Tree,
            renderer: &iced::Renderer,
            limits: &core::layout::Limits,
        ) -> core::layout::Node {
            self.content.as_widget_mut().layout(tree, renderer, limits)
        }
        fn draw(
            &self,
            tree: &core::widget::Tree,
            renderer: &mut iced::Renderer,
            theme: &iced::Theme,
            style: &core::renderer::Style,
            layout: core::Layout<'_>,
            cursor: core::mouse::Cursor,
            viewport: &Rectangle,
        ) {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
        fn operate(
            &mut self,
            tree: &mut core::widget::Tree,
            layout: core::Layout<'_>,
            renderer: &iced::Renderer,
            operation: &mut dyn core::widget::Operation,
        ) {
            // Unlike the production inert ModalFence, this shield MUST traverse.
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        }
        fn update(
            &mut self,
            tree: &mut core::widget::Tree,
            event: &iced::Event,
            layout: core::Layout<'_>,
            _: core::mouse::Cursor,
            renderer: &iced::Renderer,
            clipboard: &mut dyn core::Clipboard,
            shell: &mut core::Shell<'_, Message>,
            viewport: &Rectangle,
        ) {
            if !capture_input(event, &self.counts, shell) {
                // Redraw still reaches the real Scroller and can produce on_scroll.
                self.content.as_widget_mut().update(
                    tree,
                    event,
                    layout,
                    core::mouse::Cursor::Unavailable,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            }
        }
        fn mouse_interaction(
            &self,
            _: &core::widget::Tree,
            _: core::Layout<'_>,
            _: core::mouse::Cursor,
            _: &Rectangle,
            _: &iced::Renderer,
        ) -> core::mouse::Interaction {
            core::mouse::Interaction::None
        }
        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut core::widget::Tree,
            layout: core::Layout<'b>,
            renderer: &iced::Renderer,
            viewport: &Rectangle,
            translation: Vector,
        ) -> Option<core::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
            let counts = self.counts.clone();
            self.content
                .as_widget_mut()
                .overlay(tree, layout, renderer, viewport, translation)
                .map(|content| {
                    core::overlay::Element::new(Box::new(OverlayShield { content, counts }))
                })
        }
    }
    // Iced dispatches overlay events separately; delegating them without a shield
    // would leave an input hole even though the root widget captures physical input.
    struct OverlayShield<'a> {
        content: core::overlay::Element<'a, Message, iced::Theme, iced::Renderer>,
        counts: ShieldCounts,
    }
    impl core::Overlay<Message, iced::Theme, iced::Renderer> for OverlayShield<'_> {
        fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> core::layout::Node {
            self.content.as_overlay_mut().layout(renderer, bounds)
        }
        fn draw(
            &self,
            renderer: &mut iced::Renderer,
            theme: &iced::Theme,
            style: &core::renderer::Style,
            layout: core::Layout<'_>,
            cursor: core::mouse::Cursor,
        ) {
            self.content
                .as_overlay()
                .draw(renderer, theme, style, layout, cursor);
        }
        fn operate(
            &mut self,
            layout: core::Layout<'_>,
            renderer: &iced::Renderer,
            operation: &mut dyn core::widget::Operation,
        ) {
            self.content
                .as_overlay_mut()
                .operate(layout, renderer, operation);
        }
        fn update(
            &mut self,
            event: &iced::Event,
            layout: core::Layout<'_>,
            _: core::mouse::Cursor,
            renderer: &iced::Renderer,
            clipboard: &mut dyn core::Clipboard,
            shell: &mut core::Shell<'_, Message>,
        ) {
            if !capture_input(event, &self.counts, shell) {
                self.content.as_overlay_mut().update(
                    event,
                    layout,
                    core::mouse::Cursor::Unavailable,
                    renderer,
                    clipboard,
                    shell,
                );
            }
        }
        fn mouse_interaction(
            &self,
            _: core::Layout<'_>,
            _: core::mouse::Cursor,
            _: &iced::Renderer,
        ) -> core::mouse::Interaction {
            core::mouse::Interaction::None
        }
        fn overlay<'b>(
            &'b mut self,
            layout: core::Layout<'b>,
            renderer: &iced::Renderer,
        ) -> Option<core::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
            let counts = self.counts.clone();
            self.content
                .as_overlay_mut()
                .overlay(layout, renderer)
                .map(|content| {
                    core::overlay::Element::new(Box::new(OverlayShield { content, counts }))
                })
        }
        fn index(&self) -> f32 {
            self.content.as_overlay().index()
        }
    }

    const INITIAL: Size = Size::new(1040.0, 800.0);
    // 760 still leaves the production human bubble at its 640px width cap.
    // The parent selected 650 to exercise actual paragraph-height reflow.
    const REFLOWED: Size = Size::new(650.0, 800.0);
    const PHASES: [&str; 5] = [
        "initial-reader",
        "applicationFrameReflow",
        "prepend-older",
        "open-decision",
        "cancel-decision",
    ];

    #[derive(Clone)]
    struct Options {
        report: PathBuf,
        screenshot: Option<PathBuf>,
        phase_log: Option<PathBuf>,
        rows: usize,
        cycles: usize,
        idle_ms: u64,
    }
    impl Options {
        fn reflows(&self) -> usize {
            self.cycles * 2 - 1
        }
        fn pipeline_phase(&self, operation: usize) -> usize {
            match operation {
                0 => 0,
                n if n <= self.reflows() => 1,
                n => n - self.reflows() + 1,
            }
        }
        fn frame(&self, operation: usize) -> Size {
            if operation == 0 || (operation <= self.reflows() && operation % 2 == 0) {
                INITIAL
            } else {
                REFLOWED
            }
        }
    }
    fn new_output(value: String) -> Result<PathBuf, &'static str> {
        let path = PathBuf::from(value);
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            || path
                .to_str()
                .is_none_or(|s| s.bytes().any(|b| b < 32 || b == 127))
        {
            return Err("output-must-be-absolute-without-traversal");
        }
        let name = path.file_name().ok_or("output-name-required")?;
        let parent = path.parent().ok_or("output-parent-required")?;
        // Reject symlinks in every existing component, not merely the final filename.
        let mut ancestor = PathBuf::new();
        for component in parent.components() {
            ancestor.push(component.as_os_str());
            let metadata =
                fs::symlink_metadata(&ancestor).map_err(|_| "output-parent-unavailable")?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("output-parent-must-have-no-symlink-components");
            }
        }
        let parent = parent
            .canonicalize()
            .map_err(|_| "output-parent-unavailable")?;
        let metadata = fs::metadata(&parent).map_err(|_| "output-parent-unavailable")?;
        let uid = fs::metadata("/proc/self")
            .map_err(|_| "own-uid-unavailable")?
            .uid();
        if metadata.uid() != uid || metadata.permissions().mode() & 0o7777 != 0o700 {
            return Err("output-parent-must-be-owned-private-0700");
        }
        let path = parent.join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(path),
            _ => Err("output-must-be-new"),
        }
    }
    fn arguments(
        args: impl IntoIterator<Item = String>,
    ) -> Result<BTreeMap<String, String>, &'static str> {
        let mut args = args.into_iter();
        let mut values = BTreeMap::new();
        while let Some(key) = args.next() {
            if ![
                "--report",
                "--screenshot",
                "--phase-log",
                "--rows",
                "--cycles",
                "--idle-ms",
            ]
            .contains(&key.as_str())
            {
                return Err("unknown-argument");
            }
            let value = args.next().ok_or("argument-value-required")?;
            if values.insert(key, value).is_some() {
                return Err("duplicate-argument");
            }
        }
        Ok(values)
    }
    fn numeric(
        value: Option<String>,
        default: u64,
        minimum: u64,
        maximum: u64,
    ) -> Result<u64, &'static str> {
        let Some(value) = value else {
            return Ok(default);
        };
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("numeric-argument-must-be-unsigned-decimal");
        }
        let number = value
            .parse::<u64>()
            .map_err(|_| "numeric-argument-out-of-range")?;
        if !(minimum..=maximum).contains(&number) {
            return Err("numeric-argument-out-of-range");
        }
        Ok(number)
    }
    fn options() -> Result<Options, &'static str> {
        let mut values = arguments(std::env::args().skip(1))?;
        let rows = numeric(values.remove("--rows"), 80, 80, 4084)? as usize;
        let cycles = numeric(values.remove("--cycles"), 1, 1, 32)? as usize;
        let idle_ms = numeric(values.remove("--idle-ms"), 0, 0, 5000)?;
        let report = new_output(values.remove("--report").ok_or("report-required")?)?;
        let screenshot = values.remove("--screenshot").map(new_output).transpose()?;
        let phase_log = values.remove("--phase-log").map(new_output).transpose()?;
        if screenshot.as_ref() == Some(&report)
            || phase_log.as_ref() == Some(&report)
            || phase_log
                .as_ref()
                .is_some_and(|path| screenshot.as_ref() == Some(path))
        {
            return Err("distinct-output-files-required");
        }
        Ok(Options {
            report,
            screenshot,
            phase_log,
            rows,
            cycles,
            idle_ms,
        })
    }

    #[cfg(test)]
    mod cli_tests {
        use super::*;

        #[test]
        fn numeric_defaults_and_inclusive_bounds() {
            for (default, minimum, maximum) in [(80, 80, 4084), (1, 1, 32), (0, 0, 5000)] {
                assert_eq!(numeric(None, default, minimum, maximum), Ok(default));
                for value in [minimum, maximum] {
                    assert_eq!(
                        numeric(Some(value.to_string()), default, minimum, maximum),
                        Ok(value)
                    );
                }
                assert!(
                    numeric(Some((maximum + 1).to_string()), default, minimum, maximum).is_err()
                );
                if minimum > 0 {
                    assert!(
                        numeric(Some((minimum - 1).to_string()), default, minimum, maximum)
                            .is_err()
                    );
                }
            }
        }

        #[test]
        fn numeric_rejects_non_decimal_and_overflow() {
            for value in [
                "",
                "-1",
                "+80",
                " 80",
                "80 ",
                "1.0",
                "1e2",
                "0x50",
                "１２",
                "18446744073709551616",
            ] {
                assert!(
                    numeric(Some(value.into()), 80, 80, 4084).is_err(),
                    "{value:?}"
                );
            }
        }

        #[test]
        fn all_cli_options_reject_duplicates_and_missing_values() {
            for key in [
                "--report",
                "--screenshot",
                "--phase-log",
                "--rows",
                "--cycles",
                "--idle-ms",
            ] {
                assert_eq!(
                    arguments([key, "80", key, "80"].map(String::from)),
                    Err("duplicate-argument")
                );
                assert_eq!(
                    arguments([key].map(String::from)),
                    Err("argument-value-required")
                );
            }
            assert_eq!(
                arguments(["--unknown", "80"].map(String::from)),
                Err("unknown-argument")
            );
        }

        #[test]
        fn cycles_preserve_default_pipeline_and_always_finish_narrow() {
            for cycles in [1, 2, 32] {
                let options = Options {
                    report: PathBuf::new(),
                    screenshot: None,
                    phase_log: None,
                    rows: 80,
                    cycles,
                    idle_ms: 0,
                };
                assert_eq!(options.reflows(), cycles * 2 - 1);
                assert_eq!(options.pipeline_phase(0), 0);
                assert_eq!(options.frame(0), INITIAL);
                for operation in 1..=options.reflows() {
                    assert_eq!(options.pipeline_phase(operation), 1);
                    assert_eq!(
                        options.frame(operation),
                        if operation % 2 == 0 {
                            INITIAL
                        } else {
                            REFLOWED
                        }
                    );
                }
                for phase in 2..=4 {
                    let operation = options.reflows() + phase - 1;
                    assert_eq!(options.pipeline_phase(operation), phase);
                    assert_eq!(options.frame(operation), REFLOWED);
                }
            }
        }
    }

    fn write_phase_marker(
        file: &mut fs::File,
        boot_started: Instant,
        stage: &'static str,
        phase: Option<usize>,
        cycle: Option<usize>,
    ) -> std::io::Result<()> {
        let marker = json!({"unixTimestampNs":SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_nanos()),"elapsedNs":boot_started.elapsed().as_nanos(),"stage":stage,"phase":phase,"cycle":cycle});
        serde_json::to_writer(&mut *file, &marker).map_err(std::io::Error::other)?;
        file.write_all(b"\n")
    }

    fn near(a: f32, b: f32) -> bool {
        a.is_finite() && b.is_finite() && (a - b).abs() <= 1.0
    }
    fn number(model: &Value, key: &str) -> f32 {
        model[key].as_f64().map_or(f32::NAN, |n| n as f32)
    }
    fn scroll_metadata(model: &Value) -> Value {
        json!({"generation":model["generation"],"revision":model["revision"],"feedback":model["feedback"],"offset":model["offset"],"viewport":model["viewport"],"followBottom":model["followBottom"],"active":model["active"]})
    }
    fn ignored_category(message: &ui::Message) -> &'static str {
        match message {
            ui::Message::Editor(_) => "editor",
            ui::Message::Send => "send",
            ui::Message::Stop => "stop",
            ui::Message::Resized(_) => "app-resize",
            ui::Message::MotionFrame(_) => "motion-frame",
            ui::Message::WindowActive(_) => "window-active",
            ui::Message::Settings(_) => "settings",
            ui::Message::Interaction(_) => "interaction",
            ui::Message::Worker(_) => "worker",
            _ => "other-control",
        }
    }
    fn rect_valid(r: Rectangle) -> bool {
        [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
            && r.width > 0.0
            && r.height > 0.0
    }
    fn rect_json(r: Rectangle) -> Value {
        json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height})
    }

    #[derive(Debug, Clone, Copy)]
    struct Measurement {
        viewport: Rectangle,
        content: Rectangle,
        translation: Vector,
    }
    impl Measurement {
        fn valid(self) -> bool {
            rect_valid(self.viewport)
                && rect_valid(self.content)
                && self.translation.x.is_finite()
                && self.translation.y.is_finite()
                && self.translation.y >= -1.0
                && self.translation.y <= (self.content.height - self.viewport.height).max(0.0) + 1.0
        }
        fn json(self) -> Value {
            json!({"viewport":rect_json(self.viewport),"content":rect_json(self.content),
                "translation":{"x":self.translation.x,"y":self.translation.y},
                "maximumOffset":(self.content.height-self.viewport.height).max(0.0),"valid":self.valid()})
        }
    }
    #[derive(Debug, Clone, Default)]
    struct Observation {
        current: Option<Measurement>,
        previous: Option<Measurement>,
        current_matches: usize,
        previous_matches: usize,
        anchor_row: Option<Rectangle>,
        anchor_matches: usize,
        application_frame: Option<Rectangle>,
        frame_matches: usize,
    }
    struct Observe {
        current: Id,
        previous: Option<Id>,
        anchor: Id,
        observation: Observation,
    }
    impl Operation<Observation> for Observe {
        fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<Observation>)) {
            visit(self);
        }
        fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
            if id == Some(&Id::from("public-controlled-app-frame")) {
                self.observation.frame_matches += 1;
                self.observation.application_frame = Some(bounds);
            }
            if id == Some(&self.anchor) {
                self.observation.anchor_matches += 1;
                self.observation.anchor_row = Some(bounds);
            }
        }
        fn scrollable(
            &mut self,
            id: Option<&Id>,
            viewport: Rectangle,
            content: Rectangle,
            translation: Vector,
            _: &mut dyn Scrollable,
        ) {
            let measurement = Measurement {
                viewport,
                content,
                translation,
            };
            if id == Some(&self.current) {
                self.observation.current_matches += 1;
                self.observation.current = Some(measurement);
            } else if self
                .previous
                .as_ref()
                .is_some_and(|previous| id == Some(previous))
            {
                self.observation.previous_matches += 1;
                self.observation.previous = Some(measurement);
            }
        }
        fn finish(&self) -> Outcome<Observation> {
            // Read-only absence observation is NOT a production ScrollApplied ACK.
            // Inactive dialog views may omit both the new ID and the retained old widget.
            Outcome::Some(self.observation.clone())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Receipt {
        generation: u64,
        revision: u64,
        feedback: u64,
    }
    impl Receipt {
        fn model(model: &Value) -> Option<Self> {
            Some(Self {
                generation: model["generation"].as_u64()?,
                revision: model["revision"].as_u64()?,
                feedback: model["feedback"].as_u64()?,
            })
        }
        fn message(message: &ui::Message) -> Option<Self> {
            match message {
                ui::Message::ScrollApplied {
                    generation,
                    revision: Some(revision),
                    feedback: Some(feedback),
                    ..
                } => Some(Self {
                    generation: *generation,
                    revision: *revision,
                    feedback: *feedback,
                }),
                _ => None,
            }
        }
    }
    #[derive(Debug, Clone)]
    struct Pending {
        phase: usize,
        id: String,
        previous: Option<String>,
        receipt: Option<Receipt>,
        callback: Option<(f32, f32)>,
        anchor_key: u64,
        started: Instant,
        applied: Option<Instant>,
    }
    fn observe(pending: Pending) -> Task<Message> {
        iced::advanced::widget::operate(Observe {
            current: Id::from(pending.id.clone()),
            previous: pending.previous.clone().map(Id::from),
            anchor: Id::from(format!(
                "native-public-transcript-row-{}",
                pending.anchor_key
            )),
            observation: Observation::default(),
        })
        .map(move |o| Message::Observed(pending.clone(), o))
    }
    #[derive(Debug, Clone)]
    enum Stage {
        Boot,
        Applied(Pending),
        Observing(Pending),
        AwaitBackend,
        Idle(Instant),
        Painting,
        Finished,
    }
    #[derive(Clone)]
    enum Message {
        App(ui::Message),
        Opened(window::Id),
        Resized(window::Id, Size),
        Native(window::Id, &'static str, f32, Size),
        Observed(Pending, Observation),
        Capture(window::Id),
        Screenshot(window::Screenshot),
        Saved(bool, [u32; 2]),
        IdleDone,
        Deadline,
        Close(window::Id),
        Closed(window::Id),
    }
    impl std::fmt::Debug for Message {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("PublicScrollAppFixtureMessage(redacted)")
        }
    }
    struct Probe {
        rig: ui::scroll_fixture::Rig,
        options: Options,
        window: Option<window::Id>,
        stage: Stage,
        last_id: Option<String>,
        backend_ready: bool,
        drained_commands: usize,
        callbacks: Vec<u32>,
        boot_started: Instant,
        phase_log: Option<fs::File>,
        phase_log_failed: bool,
        ignored_callbacks: u32,
        shield_counts: ShieldCounts,
        report: Value,
    }
    impl Probe {
        fn boot(options: Options) -> (Self, Task<Message>) {
            let boot_started = Instant::now();
            let deadline = Duration::from_millis(
                20_000 + options.idle_ms + 2_000 * (options.cycles as u64 - 1),
            );
            let phase_log = options
                .phase_log
                .as_ref()
                .map(|path| {
                    fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(path)
                })
                .transpose();
            let mut phase_log_failed = phase_log.is_err();
            let mut phase_log = phase_log.ok().flatten();
            if let Some(file) = &mut phase_log {
                phase_log_failed |=
                    write_phase_marker(file, boot_started, "boot", None, None).is_err();
            }
            let mut probe = Self {
                rig: ui::scroll_fixture::Rig::with_rows(options.rows),
                callbacks: vec![0; options.reflows() + 4],
                options,
                boot_started,
                phase_log,
                phase_log_failed,
                window: None,
                stage: Stage::Boot,
                last_id: None,
                backend_ready: false,
                drained_commands: 0,
                ignored_callbacks: 0,
                shield_counts: Arc::new(std::array::from_fn(|_| AtomicU64::new(0))),
                report: json!({"scope":"PUBLIC actual production App.view/update and native widget operations in a controlled application frame inside an owned native Iced window; scripted zero-effect frame reflow, NOT native-window resizing", "publicSource":true,"zeroBusinessEffects":true,"effectsDispatched":0,"ignoredNonScrollAppMessageCount":0,"ignoredAppCategories":{},"scrollTrace":[],"resizeTrace":[],"traceLimit":32,"inputShield":{"enabled":true,"scope":"fixture widget and recursive overlays only; no physical-input qualification or compositor authority","blocks":["mouse","keyboard","touch","input-method"],"preserves":["window-events","redraw","layout","drawing","widget-operation-traversal"]},"workerStarted":false,"hostStarted":false,"pid":std::process::id(),"rendererRequested":"wgpu","applicationScale":1.0,"font":"DejaVu Sans","theme":"RosePine","steps":[],"anchorValidation":"production model key/intra plus independent native raw-slot expectedAnchorKey bounds minus viewport origin and native translation; PUBLIC feature-only raw-slot IDs", "applicationFrameSizes":[[1040,800],[650,800]],"ownedWindowRequestedSize":[1040,800],"nativeWindowResizeQualified":false,"windowSizeNegotiationQualified":false,"nativeResizeEventsForwardedToApp":false,"physicalInputQualified":false,"realHostQualified":false,"integratedPerformanceQualified":false,"capture":{"desktopCapture":false,"source":"own-Iced-renderer-buffer"}}),
            };
            probe.report["workload"] = json!({"initialRows":probe.options.rows,"cycles":probe.options.cycles,"controlledReflowCount":probe.options.reflows(),"idleMsRequested":probe.options.idle_ms,"cycleMeaning":"initial wide frame followed by narrow; additional cycles wide then narrow"});
            probe.report["timingScope"] = json!({"clock":"std::time::Instant","operationStart":"before scripted Rig/App update","applied":"exact owned ScrollApplied driver delivery before update/diagnostics","nativeObservation":"subsequent read-only native widget operation delivery","compositorPresentation":false,"physicalInput":false,"includesDiagnosticOverhead":true,"diagnosticOverhead":"model validation, report construction and optional synchronous phase-log file I/O; not subtracted"});
            probe.report["idle"] = json!({"requestedMs":probe.options.idle_ms,"durationNs":null,"afterAllNativeOperationsObserved":true,"purpose":"external resource sampling only, never restoration"});
            probe.report["phaseLog"] = json!({"enabled":probe.options.phase_log.is_some(),"diagnosticSynchronousFileIO":probe.options.phase_log.is_some()});
            probe.phase_marker("fixture-ready", None);
            (
                probe,
                Task::perform(async move { tokio::time::sleep(deadline).await }, |_| {
                    Message::Deadline
                }),
            )
        }
        fn phase_marker(&mut self, stage: &'static str, operation: Option<usize>) {
            let phase = operation.map(|n| self.options.pipeline_phase(n));
            let cycle = operation
                .filter(|n| *n > 0 && *n <= self.options.reflows())
                .map(|n| n / 2 + 1);
            if let Some(file) = &mut self.phase_log {
                self.phase_log_failed |=
                    write_phase_marker(file, self.boot_started, stage, phase, cycle).is_err();
            }
        }
        fn stage_metadata(&self) -> Value {
            let (name, phase) = match &self.stage {
                Stage::Boot => ("boot", None),
                Stage::Applied(p) => ("await-applied", Some(p.phase)),
                Stage::Observing(p) => ("observing", Some(p.phase)),
                Stage::AwaitBackend => ("await-backend", None),
                Stage::Idle(_) => ("idle-hold", None),
                Stage::Painting => ("paint-only", None),
                Stage::Finished => ("finished", None),
            };
            json!({"name":name,"operationIndex":phase,"phase":phase.map(|n| self.options.pipeline_phase(n))})
        }
        fn trace(&mut self, key: &str, value: Value) {
            let traces = self.report[key].as_array_mut().unwrap();
            if traces.len() < 32 {
                traces.push(value);
            }
        }
        fn pending(&self, phase: usize, started: Instant) -> Pending {
            Pending {
                phase,
                id: self.rig.id(),
                previous: self.last_id.clone(),
                receipt: Receipt::model(&self.rig.scalar_model()),
                callback: None,
                anchor_key: self.rig.expected_anchor_key(),
                started,
                applied: None,
            }
        }
        fn await_applied(
            &mut self,
            phase: usize,
            started: Instant,
            task: Task<ui::Message>,
        ) -> Task<Message> {
            // Count only the exact receipt owned by this pending scripted operation.
            self.callbacks[phase] = 0;
            self.stage = Stage::Applied(self.pending(phase, started));
            self.phase_marker("await-applied", Some(phase));
            task.map(Message::App)
        }
        fn commands_empty(&mut self) -> bool {
            // Rig returns its cumulative number of received commands.
            self.drained_commands = self.rig.drain_commands();
            self.drained_commands == 0 && self.rig.scalar_model()["commandCount"] == 0
        }
        fn finish(&mut self, passed: bool, reason: &str) -> Task<Message> {
            let empty = self.commands_empty();
            let final_model = self.rig.model();
            let reflows = self.options.reflows();
            let pipeline_callbacks = [
                self.callbacks[0],
                self.callbacks[reflows],
                self.callbacks[reflows + 1],
                self.callbacks[reflows + 2],
                self.callbacks[reflows + 3],
            ];
            let reflow_callbacks = &self.callbacks[1..=reflows];
            let final_stable = final_model["anchorKey"] == final_model["expectedAnchorKey"]
                && final_model["expectedAnchorKey"] == self.rig.expected_anchor_key()
                && final_model["initialRows"] == self.options.rows
                && final_model["keysCount"] == self.options.rows + 12
                && near(number(&final_model, "windowWidth"), REFLOWED.width)
                && near(number(&final_model, "windowHeight"), REFLOWED.height)
                && near(number(&final_model, "anchorIntra"), 12.0)
                && final_model["active"] == true
                && final_model["followBottom"] == false
                && self.last_id.as_ref().is_some_and(|id| *id == self.rig.id());
            let passed = passed
                && empty
                && final_stable
                && pipeline_callbacks == [1, 1, 1, 0, 1]
                && reflow_callbacks.iter().all(|count| *count == 1)
                && self.report["steps"].as_array().is_some_and(|steps| {
                    steps.len() == reflows + 4 && steps.iter().all(|step| step["passed"] == true)
                })
                && self.backend_ready
                && self.report["idle"]["durationNs"].is_number();
            self.report["callbackCounts"] = json!(pipeline_callbacks);
            self.report["reflowCallbackCounts"] = json!(reflow_callbacks);
            self.report["operationCallbackCounts"] = json!(self.callbacks);
            self.phase_marker("finished", None);
            if let Some(file) = &self.phase_log {
                self.phase_log_failed |= file.sync_all().is_err();
            }
            let passed = passed && !self.phase_log_failed;
            self.report["phaseLog"]["failed"] = json!(self.phase_log_failed);
            self.report["finalAnchorAndIdStable"] = json!(final_stable);
            let deliveries = self
                .shield_counts
                .each_ref()
                .map(|count| count.load(Ordering::Relaxed));
            self.report["inputShield"]["blockedDeliveries"] = json!(deliveries.iter().sum::<u64>());
            self.report["inputShield"]["categories"] = json!({"mouse":deliveries[0],"keyboard":deliveries[1],"touch":deliveries[2],"inputMethod":deliveries[3]});
            self.stage = Stage::Finished;
            self.report["status"] = json!(if passed { "passed" } else { "failed" });
            self.report["reason"] = json!(reason);
            self.report["ignoredCallbackCount"] = json!(self.ignored_callbacks);
            self.report["commandCount"] = final_model["commandCount"].clone();
            self.report["drainedCommandCount"] = json!(self.drained_commands);
            self.report["finalModel"] = final_model;
            self.report["fixtureMetrics"] = self.rig.metrics();
            let saved = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&self.options.report)
                .and_then(|file| {
                    serde_json::to_writer_pretty(&file, &self.report)
                        .map_err(std::io::Error::other)?;
                    file.sync_all()
                })
                .is_ok();
            if !saved {
                eprintln!("PUBLIC actual-App scroll report could not be created");
            }
            self.window.map_or_else(iced::exit, window::close)
        }
        fn start_frame_reflow(&mut self, operation: usize) -> Task<Message> {
            if self.window.is_none() {
                return self.finish(false, "owned-window-missing");
            }
            // Controlled App frame, never forwarded native Window::Resized.
            let size = self.options.frame(operation);
            let model_before = scroll_metadata(&self.rig.scalar_model());
            self.phase_marker("operation-begin", Some(operation));
            let started = Instant::now();
            let task = self.rig.update(ui::Message::Resized(size));
            let metadata = json!({"source":"scripted App Message::Resized within fixed-width native container","operationIndex":operation,"cycle":operation/2+1,"width":size.width,"height":size.height,"modelBefore":model_before,"modelAfter":scroll_metadata(&self.rig.scalar_model()),"nativeWindowResize":false});
            // Keep the original single-reflow field as the final narrow reflow.
            self.report["applicationFrameReflow"] = metadata.clone();
            if !self.report["applicationFrameReflows"].is_array() {
                self.report["applicationFrameReflows"] = json!([]);
            }
            self.report["applicationFrameReflows"]
                .as_array_mut()
                .unwrap()
                .push(metadata);
            self.await_applied(operation, started, task)
        }
        fn next(&mut self, operation: usize) -> Task<Message> {
            let reflows = self.options.reflows();
            match operation {
                0 if !self.backend_ready => {
                    self.stage = Stage::AwaitBackend;
                    self.phase_marker("await-backend", None);
                    Task::none()
                }
                n if n < reflows => self.start_frame_reflow(n + 1),
                n if n == reflows => {
                    self.phase_marker("operation-begin", Some(n + 1));
                    let started = Instant::now();
                    let task = self.rig.prepend();
                    self.await_applied(n + 1, started, task)
                }
                n if n == reflows + 1 => {
                    self.phase_marker("operation-begin", Some(n + 1));
                    let started = Instant::now();
                    let task = self.rig.open_decision();
                    let pending = self.pending(n + 1, started);
                    self.stage = Stage::Observing(pending.clone());
                    self.phase_marker("observing", Some(n + 1));
                    // Inactive absence is not a fabricated App acknowledgment.
                    Task::batch([task.map(Message::App), observe(pending)])
                }
                n if n == reflows + 2 => {
                    self.phase_marker("operation-begin", Some(n + 1));
                    let started = Instant::now();
                    let task = self.rig.cancel_decision();
                    self.await_applied(n + 1, started, task)
                }
                n if n == reflows + 3 => {
                    self.report["allNativeOperationsObserved"] = json!(true);
                    self.phase_marker("idle-begin", None);
                    let started = Instant::now();
                    self.stage = Stage::Idle(started);
                    if self.options.idle_ms == 0 {
                        self.end_idle()
                    } else {
                        let duration = Duration::from_millis(self.options.idle_ms);
                        Task::perform(async move { tokio::time::sleep(duration).await }, |_| {
                            Message::IdleDone
                        })
                    }
                }
                _ => self.finish(false, "unexpected-phase"),
            }
        }
        fn end_idle(&mut self) -> Task<Message> {
            let Stage::Idle(started) = self.stage else {
                return self.finish(false, "idle-without-completed-operations");
            };
            self.report["idle"]["durationNs"] = json!(started.elapsed().as_nanos());
            self.phase_marker("idle-end", None);
            if self.options.screenshot.is_some() {
                self.stage = Stage::Painting;
                self.phase_marker("paint-only", None);
                let id = self.window.unwrap();
                // All native restoration/observation and sampling hold already completed.
                Task::perform(
                    async { tokio::time::sleep(Duration::from_millis(250)).await },
                    move |_| Message::Capture(id),
                )
            } else {
                self.finish(true, "all-native-phases-observed")
            }
        }
        fn record(
            &mut self,
            pending: Pending,
            observation: Observation,
            observed_at: Instant,
        ) -> Task<Message> {
            self.phase_marker("native-observed", Some(pending.phase));
            let phase = self.options.pipeline_phase(pending.phase);
            let model = self.rig.model();
            let offset = number(&model, "offset");
            let intra = number(&model, "anchorIntra");
            let active = phase != 3;
            let fresh = pending.previous.as_ref().is_none_or(|id| *id != pending.id);
            let identity = self.rig.id() == pending.id && Receipt::model(&model) == pending.receipt;
            let anchor = model["anchorKey"] == pending.anchor_key
                && model["expectedAnchorKey"] == pending.anchor_key
                && near(intra, 12.0)
                && model["followBottom"] == false;
            let current = observation.current;
            let native = current.or(if active { None } else { observation.previous });
            // Pinned Scrollable::operate traverses untranslated child layout bounds.
            // Subtract the viewport origin and the actual widget's effective translation.
            let relative_top = native
                .zip(observation.anchor_row)
                .map(|(m, row)| row.y - m.viewport.y - m.translation.y);
            let native_intra = relative_top.map(|top| -top);
            let error = native.map(|m| (m.translation.y - offset).abs());
            let row_ok = if active {
                observation.anchor_matches == 1
                    && observation.anchor_row.is_some_and(|row| {
                        rect_valid(row) && near(row.height, number(&model, "anchorHeight"))
                    })
                    && relative_top.is_some_and(|top| near(top, -12.0))
                    && native_intra.is_some_and(|n| near(n, intra))
            } else {
                observation.anchor_matches <= 1
                    && observation.anchor_row.is_none_or(rect_valid)
                    && relative_top.is_none_or(|top| near(top, -12.0))
            };
            let measurement_ok = native.is_none_or(|m| {
                m.valid()
                    && near(m.translation.y, offset)
                    && near(m.viewport.height, number(&model, "viewport"))
            }) && row_ok;
            let expected_size = self.options.frame(pending.phase);
            let frame_ok = near(number(&model, "windowWidth"), expected_size.width)
                && near(number(&model, "windowHeight"), expected_size.height)
                && observation.frame_matches == 1
                && observation.application_frame.is_some_and(|frame| {
                    rect_valid(frame)
                        && near(frame.width, expected_size.width)
                        && near(frame.height, expected_size.height)
                        && native.is_none_or(|m| {
                            m.viewport.x >= frame.x - 1.0
                                && m.viewport.y >= frame.y - 1.0
                                && m.viewport.x + m.viewport.width <= frame.x + frame.width + 1.0
                                && m.viewport.y + m.viewport.height <= frame.y + frame.height + 1.0
                        })
                });
            let keys_ok = model["initialRows"] == self.options.rows
                && model["keysCount"] == self.options.rows + if phase < 2 { 0 } else { 12 };
            let previous_step = self.report["steps"]
                .as_array()
                .and_then(|steps| steps.last());
            let transition_ok = match phase {
                1 => previous_step.is_some_and(|previous| {
                    let narrow = expected_size == REFLOWED;
                    let changed = |now: f32, before: f32, increased: bool| {
                        if increased {
                            now > before + 1.0
                        } else {
                            now < before - 1.0
                        }
                    };
                    changed(
                        number(&model, "laneWidth"),
                        number(&previous["model"], "laneWidth"),
                        !narrow,
                    ) && changed(
                        number(&model, "anchorHeight"),
                        number(&previous["model"], "anchorHeight"),
                        narrow,
                    ) && changed(offset, number(&previous["model"], "offset"), narrow)
                        && current.is_some_and(|m| {
                            changed(
                                m.viewport.width,
                                number(&previous["nativeCurrent"]["viewport"], "width"),
                                !narrow,
                            )
                        })
                }),
                2 => previous_step.is_some_and(|previous| {
                    offset > number(&previous["model"], "offset") + 1.0
                        && number(&model, "totalHeight")
                            > number(&previous["model"], "totalHeight") + 1.0
                }),
                _ => true,
            };
            let callback_ok = if active {
                pending
                    .callback
                    .is_some_and(|(y, h)| near(y, offset) && near(h, number(&model, "viewport")))
                    && pending.applied.is_some()
                    && self.callbacks[pending.phase] == 1
            } else {
                pending.callback.is_none()
                    && pending.applied.is_none()
                    && self.callbacks[pending.phase] == 0
            };
            let widget_ok = if active {
                observation.current_matches == 1
                    && current.is_some()
                    && observation.previous_matches == 0
            } else {
                observation.current_matches <= 1
                    && observation.previous_matches <= 1
                    && observation.current_matches + observation.previous_matches <= 1
            };
            let passed = identity
                && fresh
                && anchor
                && model["active"] == active
                && measurement_ok
                && callback_ok
                && widget_ok
                && frame_ok
                && keys_ok
                && transition_ok
                && self.commands_empty();
            self.report["steps"].as_array_mut().unwrap().push(json!({
                "phase":phase,"operationIndex":pending.phase,"cycle":if phase == 1 {Some(pending.phase/2+1)} else {None},"name":PHASES[phase],"dynamicId":pending.id,"previousId":pending.previous,"freshDynamicId":fresh,
                "timing":{"operationToAppliedNs":pending.applied.map(|at| at.duration_since(pending.started).as_nanos()),"appliedToNativeObservationNs":pending.applied.map(|at| observed_at.duration_since(at).as_nanos()),"operationToNativeObservationNs":observed_at.duration_since(pending.started).as_nanos(),"bootToNativeObservationNs":observed_at.duration_since(self.boot_started).as_nanos(),"compositorPresentation":false,"physicalInput":false},
                "expectedAnchorKey":pending.anchor_key,
                "model":model,"nativeCurrent":current.map(Measurement::json),"nativePrevious":observation.previous.map(Measurement::json),
                "currentWidgetMatches":observation.current_matches,"previousWidgetMatches":observation.previous_matches,"nativeWidgetAbsent":native.is_none(),
                "callback":pending.callback.map(|(y,h)|json!({"offset":y,"viewport":h})),"callbackCount":self.callbacks[pending.phase],
                "offsetErrorPx":error,"nativeRelativeAnchorIntra":native_intra,"nativeAnchorRelativeTop":relative_top,
                "nativeAnchorRowBounds":observation.anchor_row.map(rect_json),"nativeAnchorRowMatches":observation.anchor_matches,
                "actualApplicationFrameBounds":observation.application_frame.map(rect_json),"applicationFrameMatches":observation.frame_matches,"applicationFrameSize":[expected_size.width,expected_size.height],"nativeWindowResize":false,
                "anchorStable":anchor,"nativeAnchorStable":row_ok,"frameValid":frame_ok,"keysCountValid":keys_ok,"transitionGeometryChanged":transition_ok,"passed":passed,
                "absentWidgetIsAppAcknowledgment":false
            }));
            if !passed {
                return self.finish(false, "phase-validation-failed");
            }
            self.last_id = Some(pending.id);
            self.next(pending.phase)
        }
        fn update(&mut self, message: Message) -> Task<Message> {
            if matches!(self.stage, Stage::Finished) {
                return if matches!(message, Message::Closed(id) if Some(id) == self.window) {
                    iced::exit()
                } else {
                    Task::none()
                };
            }
            match message {
                Message::Opened(id) if self.window.is_none() => {
                    self.window = Some(id);
                    let native =
                        window::run(id, |w| match w.display_handle().map(|h| h.as_raw()) {
                            Ok(raw_window_handle::RawDisplayHandle::Wayland(_)) => "wayland",
                            Ok(
                                raw_window_handle::RawDisplayHandle::Xlib(_)
                                | raw_window_handle::RawDisplayHandle::Xcb(_),
                            ) => "x11",
                            _ => "other-or-unavailable",
                        })
                        .then(move |backend| {
                            window::scale_factor(id).then(move |scale| {
                                window::size(id)
                                    .map(move |size| Message::Native(id, backend, scale, size))
                            })
                        });
                    if self.phase_log_failed {
                        return self.finish(false, "phase-log-unavailable");
                    }
                    self.phase_marker("operation-begin", Some(0));
                    let started = Instant::now();
                    let task = self.rig.position();
                    let position = self.await_applied(0, started, task);
                    // The runtime rebuilds the actual App view before applying its operation.
                    return Task::batch([native, position]);
                }
                Message::Native(id, backend, scale, size) if Some(id) == self.window => {
                    self.report["windowBackend"] = json!(backend);
                    self.report["nativeScaleReportedByIced"] = json!(scale);
                    self.report["initialWindowLogicalSize"] = json!([size.width, size.height]);
                    self.report["ownedWindowSizeNegotiationClaimed"] = json!(false);
                    self.report["controlledFrameFitsOwnedWindow"] = json!(
                        size.width.is_finite()
                            && size.height.is_finite()
                            && size.width >= INITIAL.width
                            && size.height >= INITIAL.height
                    );
                    if backend != "wayland"
                        || !scale.is_finite()
                        || scale <= 0.0
                        || !size.width.is_finite()
                        || !size.height.is_finite()
                        || size.width < INITIAL.width
                        || size.height < INITIAL.height
                    {
                        return self.finish(false, "initial-native-window-validation-failed");
                    }
                    self.backend_ready = true;
                    if matches!(self.stage, Stage::AwaitBackend) {
                        return self.start_frame_reflow(1);
                    }
                }
                Message::Resized(id, size) if Some(id) == self.window => {
                    // Observe native negotiation only. The scripted application frame
                    // is independent; never forward these sizes into actual App state.
                    let model = scroll_metadata(&self.rig.scalar_model());
                    self.trace("resizeTrace", json!({"stage":self.stage_metadata(),"width":size.width,"height":size.height,"forwardedToApp":false,"modelBefore":model,"modelAfter":model}));
                    self.report["latestOwnedWindowLogicalSize"] = json!([size.width, size.height]);
                    let frame = self.rig.scalar_model();
                    if !size.width.is_finite()
                        || !size.height.is_finite()
                        || size.width < number(&frame, "windowWidth")
                        || size.height < number(&frame, "windowHeight")
                    {
                        return self.finish(
                            false,
                            "controlled-application-frame-cannot-fit-owned-window",
                        );
                    }
                    return Task::none();
                }
                Message::App(message) => {
                    let delivered_at = Instant::now();
                    // Actual view controls have no authority in this scripted fixture.
                    // Resize and decision/page transitions use only the owned driver paths.
                    if !matches!(
                        &message,
                        ui::Message::Scrolled { .. } | ui::Message::ScrollApplied { .. }
                    ) {
                        let ignored = self.report["ignoredNonScrollAppMessageCount"]
                            .as_u64()
                            .unwrap_or(0);
                        self.report["ignoredNonScrollAppMessageCount"] =
                            json!(ignored.saturating_add(1));
                        let category = ignored_category(&message);
                        let count = self.report["ignoredAppCategories"][category]
                            .as_u64()
                            .unwrap_or(0);
                        self.report["ignoredAppCategories"][category] =
                            json!(count.saturating_add(1));
                        return Task::none();
                    }
                    let accepted = self.rig.current_applied(&message);
                    let receipt = Receipt::message(&message);
                    let callback = match &message {
                        ui::Message::ScrollApplied { y, viewport, .. } => Some((*y, *viewport)),
                        _ => None,
                    };
                    let scroll_trace = match &message {
                        ui::Message::Scrolled {
                            generation,
                            revision,
                            viewport,
                        } if self.report["scrollTrace"]
                            .as_array()
                            .is_some_and(|traces| traces.len() < 32) =>
                        {
                            Some(json!({
                                "stage":self.stage_metadata(),"generation":generation,"revision":revision,
                                "y":viewport.absolute_offset().y,"height":viewport.bounds().height,
                                "relativeY":viewport.relative_offset().y,"contentHeight":viewport.content_bounds().height,
                                "modelBefore":scroll_metadata(&self.rig.scalar_model())
                            }))
                        }
                        _ => None,
                    };
                    let task = self.rig.update(message);
                    if let Some(mut trace) = scroll_trace {
                        trace["modelAfter"] = scroll_metadata(&self.rig.scalar_model());
                        self.trace("scrollTrace", trace);
                    }
                    if !self.commands_empty() {
                        return self.finish(false, "unexpected-command");
                    }
                    if let Some(callback) = callback {
                        let pending = match &self.stage {
                            Stage::Applied(p)
                                if accepted && receipt == p.receipt && self.rig.id() == p.id =>
                            {
                                Some(p.clone())
                            }
                            _ => None,
                        };
                        if let Some(mut pending) = pending {
                            self.callbacks[pending.phase] += 1;
                            pending.callback = Some(callback);
                            pending.applied = Some(delivered_at);
                            self.stage = Stage::Observing(pending.clone());
                            self.phase_marker("applied", Some(pending.phase));
                            // Separate immediate read-only operation AFTER actual App receipt update.
                            return Task::batch([task.map(Message::App), observe(pending)]);
                        }
                        // Exact current receipts cannot appear twice or acknowledge an
                        // inactive dialog, even though ordinary on_scroll is forwarded.
                        if let Stage::Observing(p) = &self.stage {
                            if receipt == p.receipt && self.rig.id() == p.id {
                                let reason = if self.options.pipeline_phase(p.phase) == 3 {
                                    "inactive-dialog-false-acknowledgment"
                                } else {
                                    "duplicate-current-operation-receipt"
                                };
                                return self.finish(false, reason);
                            }
                        }
                        self.ignored_callbacks += 1;
                    }
                    // Scrolled is always forwarded but never a phase-completion receipt.
                    return task.map(Message::App);
                }
                Message::Observed(pending, observation) => {
                    let observed_at = Instant::now();
                    if matches!(&self.stage, Stage::Observing(p) if p.phase == pending.phase && p.id == pending.id && p.receipt == pending.receipt)
                    {
                        return self.record(pending, observation, observed_at);
                    }
                    // A superseded observer cannot advance the current scripted phase.
                }
                Message::Capture(id)
                    if Some(id) == self.window && matches!(self.stage, Stage::Painting) =>
                {
                    return window::screenshot(id).map(Message::Screenshot);
                }
                Message::Screenshot(screenshot) if matches!(self.stage, Stage::Painting) => {
                    let Some(path) = self.options.screenshot.clone() else {
                        return self.finish(false, "missing-screenshot-path");
                    };
                    let size = [screenshot.size.width, screenshot.size.height];
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                save_screenshot(&path, &screenshot).is_ok()
                            })
                            .await
                            .unwrap_or(false)
                        },
                        move |saved| Message::Saved(saved, size),
                    );
                }
                Message::Saved(saved, size) if matches!(self.stage, Stage::Painting) => {
                    self.report["capture"]["saved"] = json!(saved);
                    self.report["capture"]["physicalBufferSize"] = json!(size);
                    return self.finish(
                        saved,
                        if saved {
                            "all-native-phases-and-paint-observed"
                        } else {
                            "screenshot-save-failed"
                        },
                    );
                }
                Message::IdleDone if matches!(self.stage, Stage::Idle(_)) => {
                    return self.end_idle();
                }
                Message::Deadline => return self.finish(false, "deadline-before-completion"),
                Message::Close(id) if Some(id) == self.window => {
                    return self.finish(false, "owned-window-manually-closed");
                }
                Message::Closed(id) if Some(id) == self.window => {
                    self.window = None;
                    return self.finish(false, "owned-window-closed-before-completion");
                }
                _ => {}
            }
            Task::none()
        }
        fn view(&self) -> Element<'_, Message> {
            // Actual production view in a controlled frame, not a resized native
            // window. Larger compositor-owned space is unused; no clipping workaround.
            let frame = self.rig.frame();
            let content = iced::widget::container(self.rig.view().map(Message::App))
                .id("public-controlled-app-frame")
                .width(frame.width)
                .height(frame.height);
            Element::new(InputShield {
                content: content.into(),
                counts: self.shield_counts.clone(),
            })
        }
        fn subscription(&self) -> Subscription<Message> {
            // No App.subscription, worker start, physical input subscription or animation.
            window::events().filter_map(|(id, event)| match event {
                window::Event::Opened { .. } => Some(Message::Opened(id)),
                window::Event::Resized(size) => Some(Message::Resized(id, size)),
                window::Event::CloseRequested => Some(Message::Close(id)),
                window::Event::Closed => Some(Message::Closed(id)),
                _ => None,
            })
        }
    }
    fn save_screenshot(path: &Path, screenshot: &window::Screenshot) -> std::io::Result<()> {
        let size = screenshot.size;
        let bytes = (size.width as usize)
            .checked_mul(size.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| std::io::Error::other("invalid-public-buffer-size"))?;
        if bytes == 0 || bytes > 64 * 1024 * 1024 || bytes != screenshot.rgba.len() {
            return Err(std::io::Error::other("invalid-public-buffer-length"));
        }
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        let mut encoder = png::Encoder::new(&file, size.width, size.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&screenshot.rgba)?;
        writer.finish()?;
        file.sync_all()
    }
    pub fn run() -> iced::Result {
        unsafe {
            std::env::set_var("ICED_BACKEND", "wgpu");
        }
        let options = options().unwrap_or_else(|error| {
            eprintln!("{error}");
            std::process::exit(2)
        });
        let report = options.report.clone();
        let result = iced::application(
            move || Probe::boot(options.clone()),
            Probe::update,
            Probe::view,
        )
        .title("Harness actual App scroll · PUBLIC zero-Host fixture")
        .window(window::Settings {
            size: INITIAL,
            resizable: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "ai.deepseek.harness.native.public-scroll-app-fixture".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .exit_on_close_request(false)
        .settings(iced::Settings {
            default_text_size: iced::Pixels(14.0),
            ..Default::default()
        })
        .default_font(ui::FONT)
        .theme(|_: &Probe| ui::native_theme())
        .scale_factor(|_: &Probe| 1.0)
        .subscription(Probe::subscription)
        .run();
        let passed = fs::read(&report)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|v| {
                v["status"] == "passed" && v["commandCount"] == 0 && v["drainedCommandCount"] == 0
            });
        if result.is_ok() && !passed {
            eprintln!("PUBLIC actual-App scroll fixture failed; inspect report");
            std::process::exit(1);
        }
        result
    }
}

#[cfg(feature = "public-layout-fixture")]
fn main() -> iced::Result {
    driver::run()
}
#[cfg(not(feature = "public-layout-fixture"))]
fn main() {
    eprintln!("scroll_app_smoke requires --features public-layout-fixture");
    std::process::exit(2);
}
