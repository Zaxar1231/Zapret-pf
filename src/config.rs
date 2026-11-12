use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::paths;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameFilter {
    #[default]
    Off,
    Tcp,
    Udp,
    All,
}

impl GameFilter {
    pub const ALL: [GameFilter; 4] = [GameFilter::Off, GameFilter::Tcp, GameFilter::Udp, GameFilter::All];

    pub fn label(self) -> &'static str {
        match self {
            GameFilter::Off => "Выкл",
            GameFilter::Tcp => "TCP",
            GameFilter::Udp => "UDP",
            GameFilter::All => "TCP+UDP",
        }
    }

    /// Значения %GameFilterTCP% / %GameFilterUDP% как в service.bat
    /// ("12" — заглушка, когда фильтр выключен).
    pub fn ports(self) -> (&'static str, &'static str) {
        const R: &str = "1024-65535";
        match self {
            GameFilter::Off => ("12", "12"),
            GameFilter::Tcp => (R, "12"),
            GameFilter::Udp => ("12", R),
            GameFilter::All => (R, R),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "Системная",
            ThemeMode::Light => "Светлая",
            ThemeMode::Dark => "Тёмная",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct TestScore {
    pub ok: usize,
    pub total: usize,
    pub avg_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub selected_profile: Option<String>,
    pub game_filter: GameFilter,
    pub autostart: bool,
    pub autoconnect: bool,
    pub check_updates: bool,
    pub disconnect_on_exit: bool,
    /// Автоматически переключать профиль, если текущий перестал работать.
    pub auto_switch: bool,
    pub theme: ThemeMode,
    pub scores: BTreeMap<String, TestScore>,
    pub skipped_app_version: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            selected_profile: None,
            game_filter: GameFilter::Off,
            autostart: false,
            autoconnect: true,
            check_updates: true,
            disconnect_on_exit: false,
            auto_switch: true,
            theme: ThemeMode::System,
            scores: BTreeMap::new(),
            skipped_app_version: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(paths::config_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(paths::config_file(), s);
        }
    }
}
