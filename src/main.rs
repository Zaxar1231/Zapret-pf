#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod paths;
mod profiles;
mod sys;
mod tester;
mod theme;
mod updater;
mod widgets;
mod zapret;

/// Иконка окна рисуется программно: кнопка питания на синем круге.
fn window_icon() -> egui::IconData {
    let n = 64u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            let i = ((y * n + x) * 4) as usize;
            let a = (c - d).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let ang = dy.atan2(dx); // -PI..PI, верх = -PI/2
            let ring = (d - 17.0).abs() < 3.0 && !(ang < -1.0 && ang > -2.14);
            let bar = dx.abs() < 3.0 && dy < -2.0 && dy > -23.0;
            let (r, g, b) = if ring || bar { (255, 255, 255) } else { (0x3F, 0x5F, 0xA9) };
            rgba[i..i + 4].copy_from_slice(&[r, g, b, (a * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: n, height: n }
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
