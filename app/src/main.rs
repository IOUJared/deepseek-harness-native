mod codex;
mod config;
mod design;
mod export_file;
mod exporter;
mod interactions;
mod known;
mod management;
mod plugins;
mod reducer;
mod settings;
mod smoke;
mod ui;
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
    let smoke_report = options.smoke.as_ref().map(|s| s.report.clone());
    // SAFETY: renderer selection is set before Iced/Tokio or the owner thread starts.
    unsafe {
        std::env::set_var("ICED_BACKEND", "wgpu");
    }
    let (handle, feed, owner) =
        match worker::start(options.backend.clone(), options.smoke.is_some()) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        };
    let result = iced::application(
        move || ui::App::boot(options.clone(), handle.clone(), feed.clone()),
        ui::App::update,
        ui::App::view,
    )
    .title("DeepSeek Harness · Native")
    .window(window::Settings {
        size: Size::new(1200.0, 800.0),
        min_size: Some(Size::new(760.0, 560.0)),
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
