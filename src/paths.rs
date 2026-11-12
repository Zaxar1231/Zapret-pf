//! Все файлы приложения живут рядом с исполняемым файлом (в папке установки):
//!   <install>\ZapretPF.exe
//!   <install>\zapret\        — файлы flowseal/zapret-discord-youtube (профили, bin, lists)
//!   <install>\data\          — настройки, журнал, временные файлы

use std::path::PathBuf;

pub fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn zapret_dir() -> PathBuf {
    install_dir().join("zapret")
}

pub fn bin_dir() -> PathBuf {
    zapret_dir().join("bin")
}

pub fn lists_dir() -> PathBuf {
    zapret_dir().join("lists")
}

pub fn winws_exe() -> PathBuf {
    bin_dir().join("winws.exe")
}

pub fn data_dir() -> PathBuf {
    let d = install_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn tmp_dir() -> PathBuf {
    let d = data_dir().join("tmp");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}

pub fn log_file() -> PathBuf {
    data_dir().join("winws.log")
}

pub fn zapret_installed() -> bool {
    winws_exe().exists()
}

/// Путь в стиле Windows с завершающим разделителем — как `%~dp0` в .bat.
pub fn with_trailing_sep(p: &std::path::Path) -> String {
    let mut s = p.to_string_lossy().to_string();
    if !s.ends_with('\\') && !s.ends_with('/') {
        s.push(std::path::MAIN_SEPARATOR);
    }
    s
}
