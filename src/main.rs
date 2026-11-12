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

/// Сообщение об ошибке пользователю (у приложения нет консоли).
pub fn error_box(text: &str) {
    let _ = std::fs::write(paths::data_dir().join("crash.log"), text);
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        MessageBoxW(std::ptr::null_mut(), w(text).as_ptr(), w("Zapret PF — ошибка").as_ptr(), MB_OK | MB_ICONERROR);
    }
    #[cfg(not(windows))]
    eprintln!("{text}");
}

enum Instance {
    Primary(std::net::TcpListener),
    Secondary,
    Unknown,
}

/// Единственный экземпляр. Если копия уже запущена — просим её показать окно и ждём
/// ответа «ok». Зависшую копию (не ответила) завершаем и запускаемся сами.
fn acquire_instance() -> Instance {
    use std::io::{Read, Write};
    use std::time::Duration;
    for attempt in 0..2 {
        if let Ok(l) = std::net::TcpListener::bind(("127.0.0.1", INSTANCE_PORT)) {
            return Instance::Primary(l);
        }
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], INSTANCE_PORT));
        if let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(4)));
            let _ = s.write_all(b"show\n");
            let mut buf = [0u8; 2];
            if s.read_exact(&mut buf).is_ok() && &buf == b"ok" {
                return Instance::Secondary;
            }
        }
        if attempt == 0 {
            let pid = std::process::id().to_string();
            let filter = format!("PID ne {pid}");
            let _ = sys::run("taskkill", &["/F", "/IM", "ZapretPF.exe", "/FI", &filter]);
            std::thread::sleep(Duration::from_millis(800));
        }
    }
    Instance::Unknown
}

fn renderer_file() -> std::path::PathBuf {
    paths::data_dir().join("renderer.txt")
}

fn main() {
    // Любая паника — в журнал и в окно с сообщением, а не молчаливое закрытие.
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let msg = format!("Zapret PF {} упал (поток {thread}):\n{info}\n\n{bt}", env!("CARGO_PKG_VERSION"));
        let _ = std::fs::write(paths::data_dir().join("crash.log"), &msg);
        if thread == "main" {
            error_box(&format!("{info}\n\nПодробности: {}", paths::data_dir().join("crash.log").display()));
        }
    }));

    if let Err(e) = real_main() {
        error_box(&format!(
            "Не удалось запустить окно приложения:\n{e}\n\nОбновите видеодрайвер. Журнал: {}",
            paths::data_dir().join("crash.log").display()
        ));
    }
}

fn real_main() -> eframe::Result {
    let autostart = std::env::args().any(|a| a == "--autostart");

    let listener = match acquire_instance() {
        Instance::Primary(l) => Some(l),
        Instance::Secondary => return Ok(()), // окно уже открытого приложения показано
        Instance::Unknown => None,
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
    // Рендерер: по умолчанию OpenGL (glow), он запускается почти везде. Если OpenGL
    // недоступен, запоминаем это и перезапускаемся с DirectX 12/Vulkan (wgpu): создать окно
    // второй раз в том же процессе winit не позволяет.
    let saved = std::fs::read_to_string(renderer_file()).unwrap_or_default();
    let use_wgpu = saved.trim() == "wgpu" || std::env::args().any(|a| a == "--wgpu");
    let renderer = if use_wgpu { eframe::Renderer::Wgpu } else { eframe::Renderer::Glow };
    let res = eframe::run_native(
        "Zapret PF",
        make_options(renderer),
        Box::new(|cc| Ok(Box::new(app::ZapretApp::new(cc, autostart, listener.lock().unwrap().take())))),
    );
    if let Err(e) = &res {
        let _ = std::fs::write(paths::data_dir().join("crash.log"), format!("{renderer:?}: {e}"));
        if !use_wgpu && !matches!(e, eframe::Error::AppCreation(_)) {
            let _ = std::fs::write(renderer_file(), "wgpu");
            drop(listener.lock().unwrap().take());
            if let Ok(exe) = std::env::current_exe() {
                let args: Vec<String> = std::env::args().skip(1).collect();
                if std::process::Command::new(exe).args(args).spawn().is_ok() {
                    return Ok(());
                }
            }
        }
    }
    res
}
