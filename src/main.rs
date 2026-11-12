#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod icon;
mod paths;
mod profiles;
mod sys;
mod tester;
#[cfg(windows)]
mod tray;
mod theme;
mod updater;
mod widgets;
mod zapret;

fn window_icon() -> egui::IconData {
    egui::IconData { rgba: icon::power_icon_rgba(64, icon::COLOR_ON), width: 64, height: 64 }
}

/// Порт-«замок» единственного экземпляра. Повторный запуск просит открытое окно показаться.
const INSTANCE_PORT: u16 = 47613;

fn main() -> eframe::Result {
    let autostart = std::env::args().any(|a| a == "--autostart");

    let listener = match std::net::TcpListener::bind(("127.0.0.1", INSTANCE_PORT)) {
        Ok(l) => Some(l),
        Err(_) => {
            if std::net::TcpStream::connect(("127.0.0.1", INSTANCE_PORT)).is_ok() {
                return Ok(()); // приложение уже запущено — оно покажет своё окно
            }
            None
        }
    };
    let listener = std::sync::Arc::new(std::sync::Mutex::new(listener));

    let make_options = |renderer: eframe::Renderer| eframe::NativeOptions {
        renderer,
        viewport: egui::ViewportBuilder::default()
            .with_title("Zapret PF")
            .with_app_id("ZapretPF")
            .with_inner_size([420.0, 760.0])
            .with_min_inner_size([380.0, 560.0])
            .with_icon(window_icon()),
        persist_window: true,
        persistence_path: Some(paths::data_dir().join("window.ron")),
        centered: true,
        ..Default::default()
    };
    // Сначала DirectX 12/Vulkan (wgpu), при ошибке — OpenGL.
    let run = |r| eframe::run_native("Zapret PF", make_options(r), Box::new(|cc| Ok(Box::new(app::ZapretApp::new(cc, autostart, listener.lock().unwrap().take())))));
    match run(eframe::Renderer::Wgpu) {
        Err(e) if !matches!(e, eframe::Error::AppCreation(_)) => {
            eprintln!("wgpu недоступен ({e}), пробуем OpenGL");
            run(eframe::Renderer::Glow)
        }
        r => r,
    }
}
