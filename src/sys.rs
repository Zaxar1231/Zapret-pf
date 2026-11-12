//! Работа с системой Windows: скрытый запуск процессов, автозапуск, проверки.

use std::process::{Command, Output, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[allow(dead_code)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[allow(dead_code)]
pub const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Команда без всплывающего консольного окна.
pub fn hidden(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c.stdin(Stdio::null());
    c
}

pub fn run(program: &str, args: &[&str]) -> Option<Output> {
    hidden(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).output().ok()
}

pub fn run_ok(program: &str, args: &[&str]) -> bool {
    run(program, args).map(|o| o.status.success()).unwrap_or(false)
}

/// Запущен ли процесс с указанным именем (например, `winws.exe`).
pub fn process_running(image: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let filter = format!("IMAGENAME eq {image}");
    run("tasklist", &["/FI", &filter, "/NH", "/FO", "CSV"])
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&image.to_lowercase()))
        .unwrap_or(false)
}

pub fn kill_process(image: &str) {
    if cfg!(windows) {
        let _ = run("taskkill", &["/F", "/IM", image]);
    }
}

/// Существует ли служба Windows.
pub fn service_exists(name: &str) -> bool {
    cfg!(windows) && run_ok("sc", &["query", name])
}

/// Удаляет службу zapret, установленную через service.bat, и драйвер WinDivert.
pub fn remove_zapret_service() {
    for svc in ["zapret", "WinDivert", "WinDivert14"] {
        let _ = run("net", &["stop", svc]);
        let _ = run("sc", &["delete", svc]);
    }
}

/// Остановить драйвер WinDivert, чтобы можно было заменить его файлы.
pub fn stop_windivert() {
    for svc in ["WinDivert", "WinDivert14"] {
        let _ = run("sc", &["stop", svc]);
    }
}

/// Включить TCP timestamps — нужно стратегиям с `--dpi-desync-fooling=ts`
/// (то же самое делает service.bat при каждом запуске).
pub fn enable_tcp_timestamps() {
    if cfg!(windows) {
        let _ = run("netsh", &["interface", "tcp", "set", "global", "timestamps=enabled"]);
    }
}

const TASK_NAME: &str = "ZapretPF";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Автозапуск через Планировщик заданий с наивысшими правами — без запроса UAC при входе.
/// Задача описывается XML, чтобы снять ограничения по умолчанию: лимит работы 72 часа
/// и запрет запуска от батареи.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    if !cfg!(windows) {
        return Ok(());
    }
    if !enable {
        let _ = run("schtasks", &["/Delete", "/F", "/TN", TASK_NAME]);
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(d), Ok(u)) => format!("<UserId>{}</UserId>", xml_escape(&format!("{d}\\{u}"))),
        _ => String::new(),
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Zapret PF — автозапуск</Description></RegistrationInfo>
  <Triggers>
    <LogonTrigger><Enabled>true</Enabled><Delay>PT15S</Delay>{user}</LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <Enabled>true</Enabled>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec><Command>{cmd}</Command><Arguments>--autostart</Arguments></Exec>
  </Actions>
</Task>
"#,
        cmd = xml_escape(&exe.to_string_lossy()),
    );
    // schtasks надёжно читает XML только в UTF-16 LE с BOM
    let mut bytes = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let file = crate::paths::data_dir().join("autostart.xml");
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    let out = run("schtasks", &["/Create", "/F", "/TN", TASK_NAME, "/XML", &file.to_string_lossy()])
        .ok_or("не удалось запустить schtasks")?;
    let _ = std::fs::remove_file(&file);
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn autostart_enabled() -> bool {
    cfg!(windows) && run_ok("schtasks", &["/Query", "/TN", TASK_NAME])
}

pub fn open_path(p: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(p).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("xdg-open").arg(p).spawn();
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = hidden("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}
