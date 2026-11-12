//! Запуск/остановка winws.exe с выбранным профилем.

use std::fs::OpenOptions;
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::config::GameFilter;
use crate::paths;
use crate::profiles::Profile;
use crate::sys;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub fn is_running() -> bool {
    sys::process_running("winws.exe")
}

pub fn stop() {
    sys::kill_process("winws.exe");
    // ждём, пока процесс действительно завершится
    let t = Instant::now();
    while is_running() && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Создаёт пользовательские списки, если их нет (аналог `service.bat load_user_lists`).
pub fn ensure_user_lists() {
    let l = paths::lists_dir();
    let _ = std::fs::create_dir_all(&l);
    let files: [(&str, &str); 3] = [
        ("ipset-exclude-user.txt", "203.0.113.113/32\n"),
        ("list-general-user.txt", "# Never leave this file empty\ndomain.example.abc\n"),
        ("list-exclude-user.txt", "domain.example.abc\n"),
    ];
    for (name, content) in files {
        let p = l.join(name);
        if !p.exists() {
            let _ = std::fs::write(p, content);
        }
    }
}

/// Запускает winws.exe с профилем. Процесс работает независимо от окна приложения.
pub fn start(profile: &Profile, game: GameFilter) -> Result<(), String> {
    if !paths::zapret_installed() {
        return Err("Файлы zapret не найдены. Обновите zapret в настройках.".into());
    }
    if sys::service_exists("zapret") {
        return Err("Обнаружена служба zapret из service.bat. Удалите её в настройках, чтобы избежать конфликта.".into());
    }
    if is_running() {
        stop();
    }
    ensure_user_lists();
    sys::enable_tcp_timestamps();

    let args = profile.resolved_args(game);
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(paths::log_file())
        .map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = std::process::Command::new(paths::winws_exe());
    cmd.args(&args)
        .current_dir(paths::bin_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    #[cfg(windows)]
    cmd.creation_flags(sys::CREATE_NO_WINDOW | sys::CREATE_NEW_PROCESS_GROUP);

    let mut child = cmd.spawn().map_err(|e| format!("Не удалось запустить winws.exe: {e}"))?;

    // Если процесс упал сразу — это ошибка аргументов или драйвера WinDivert.
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(1500) {
        if let Ok(Some(status)) = child.try_wait() {
            let tail = std::fs::read_to_string(paths::log_file()).unwrap_or_default();
            let tail: String = tail.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            return Err(format!("winws.exe завершился (код {:?}).\n{}", status.code(), tail));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Локальная версия zapret — из строки `set "LOCAL_VERSION=x.y.z"` в service.bat.
pub fn local_version() -> Option<String> {
    let text = std::fs::read_to_string(paths::zapret_dir().join("service.bat")).ok()?;
    for line in text.lines() {
        if let Some(idx) = line.find("LOCAL_VERSION=") {
            let v = line[idx + "LOCAL_VERSION=".len()..].trim().trim_end_matches('"').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}
