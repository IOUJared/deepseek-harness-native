mod codex;
mod config;
mod crash_log;
mod design;
mod export_file;
mod exporter;
mod fork;
mod goal_ui;
mod interactions;
mod keybindings;
mod known;
mod management;
mod mcp;
mod plugins;
mod queue_dock;
mod queue_update;
mod reducer;
mod rename;
mod secret_input;
mod settings;
mod single_instance;
mod smoke;
mod terminal;
mod ui;
mod work_panel;
mod worker;
use iced::{Size, window};
fn main() -> iced::Result {
    let options = match config::parse(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", config::USAGE);
            return Ok(());
        }
        Err(error) => {
            eprintln!("{error}\n\n{}", config::USAGE);
            std::process::exit(2);
        }
    };
    let mut options = options;
    // A real interactive run exposes its controls to AT-SPI; a smoke run renders
    // its own one-shot window and must not announce itself as an application.
    options.accessibility = options.smoke.is_none();
    let smoke_report = options.smoke.as_ref().map(|s| s.report.clone());
    // SAFETY: renderer selection is set before Iced/Tokio or the owner thread starts.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    // One live instance per profile. The macOS shell brings the window that
    // already owns the profile forward; native asks it to do the same over the
    // instance socket and only refuses when nothing answers. Smoke runs are
    // exempt, matching the stores the UI also skips under `--smoke`, so a
    // fixture pass cannot collide with a live app.
    let instance = if options.smoke.is_none() {
        match single_instance::claim(&options.backend.native_home) {
            single_instance::Outcome::Claimed(claim) => Some(claim),
            single_instance::Outcome::AlreadyRunning => {
                if single_instance::notify_owner(&options.backend.native_home) {
                    eprintln!("DeepSeek Harness is already running; asked it to come forward.");
                } else {
                    eprintln!(
                        "DeepSeek Harness is already running for this profile; quit it first."
                    );
                }
                return Ok(());
            }
            single_instance::Outcome::Unavailable => None,
        }
    } else {
        None
    };
    // The claim owns the focus socket; the App subscribes to what arrives on it.
    // It is read here because the instance decision precedes the App's creation.
    let focus_source = match &instance {
        Some(claim) => claim.focus_source(),
        None => single_instance::Source::disabled(),
    };
    let (handle, feed, owner) =
        match worker::start(options.backend.clone(), options.smoke.is_some()) {
            Ok(value) => value,
            Err(error) => {
                // The shell keeps a bounded crash report for a failed start; a
                // launch from a desktop entry otherwise leaves no trace. The
                // record never replaces the failure being reported.
                let summary = error.to_string();
                match crash_log::write(
                    &options.backend.native_home,
                    &crash_log::Record {
                        source: "host-start",
                        summary: &summary,
                        detail: &summary,
                    },
                ) {
                    Some(path) => eprintln!("{error}\nWrote {}", path.display()),
                    None => eprintln!("{error}"),
                }
                std::process::exit(1);
            }
        };
    let result = iced::application(
        move || {
            let (mut app, task) = ui::App::boot(options.clone(), handle.clone(), feed.clone());
            app.focus_source = focus_source.clone();
            (app, task)
        },
        ui::App::update,
        ui::App::view,
    )
    .title("DeepSeek Harness · Native")
    .window(window::Settings {
        // The DMG's own window options: a 1280x820 main window with a 520x600
        // minimum (`PARITY-INVENTORY.md` §Windows, from `main.js`). Native used a
        // 760x560 minimum, which no reference value backs.
        size: Size::new(design::WINDOW_WIDTH, design::WINDOW_HEIGHT),
        min_size: Some(Size::new(
            design::WINDOW_MIN_WIDTH,
            design::WINDOW_MIN_HEIGHT,
        )),
        resizable: true,
        platform_specific: window::settings::PlatformSpecific {
            application_id: "ai.deepseek.harness.native.app".into(),
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
    .theme(ui::theme)
    .scale_factor(|app: &ui::App| app.options.scale)
    .subscription(ui::App::subscription)
    .run();
    drop(owner);
    if let Some(path) = smoke_report {
        let passed = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|value| value["status"] == "passed");
        if !passed {
            eprintln!("Native smoke failed or evidence unavailable");
            std::process::exit(1);
        }
    }
    result
}
