//! Автоподбор профиля: по очереди запускаем каждый профиль и проверяем доступность
//! Discord/YouTube/Google и т.д. Это повторяет идею `utils\test zapret.ps1`.
//! Мы скачиваем до 64 КБ ответа — так ловится и блокировка «зависанием после 16 КБ».

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{GameFilter, TestScore};
use crate::paths;
use crate::profiles::Profile;
use crate::zapret;

const DEFAULT_TARGETS: &[&str] = &[
    "https://discord.com",
    "https://gateway.discord.gg",
    "https://cdn.discordapp.com",
    "https://updates.discord.com",
    "https://www.youtube.com",
    "https://i.ytimg.com",
    "https://redirector.googlevideo.com",
    "https://www.google.com",
    "https://www.cloudflare.com",
];

/// Цели из `zapret\utils\targets.txt` (только URL), иначе — список по умолчанию.
pub fn targets() -> Vec<String> {
    let file = paths::zapret_dir().join("utils").join("targets.txt");
    let mut v = Vec::new();
    if let Ok(text) = std::fs::read_to_string(file) {
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((_, val)) = line.split_once('=') {
                let val = val.trim().trim_matches('"');
                if val.starts_with("https://") || val.starts_with("http://") {
                    v.push(val.to_string());
                }
            }
        }
    }
    if v.is_empty() {
        v = DEFAULT_TARGETS.iter().map(|s| s.to_string()).collect();
    }
    v
}

fn check_target(url: &str, limit: u64) -> Option<Duration> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(7))
        .connect_timeout(Duration::from_secs(4))
        .pool_max_idle_per_host(0)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36")
        .build()
        .ok()?;
    let t = Instant::now();
    let resp = client.get(url).send().ok()?;
    let mut buf = Vec::new();
    resp.take(limit).read_to_end(&mut buf).ok()?;
    Some(t.elapsed())
}

/// Проверка всех целей параллельно при уже запущенном (или не запущенном) winws.
pub fn check_all(targets: &[String]) -> TestScore {
    check_all_limited(targets, 64 * 1024)
}

/// То же, но с ограничением объёма скачивания на цель (для фоновых проверок).
pub fn check_all_limited(targets: &[String], limit: u64) -> TestScore {
    let results: Vec<Option<Duration>> = std::thread::scope(|s| {
        let hs: Vec<_> = targets.iter().map(|t| s.spawn(move || check_target(t, limit))).collect();
        hs.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    let ok: Vec<Duration> = results.iter().flatten().copied().collect();
    let avg_ms = if ok.is_empty() { 0 } else { ok.iter().map(|d| d.as_millis() as u64).sum::<u64>() / ok.len() as u64 };
    TestScore { ok: ok.len(), total: targets.len(), avg_ms }
}

pub enum TestEvent {
    Progress { index: usize, total: usize, profile: String },
    Result { profile: String, score: TestScore },
    Done { best: Option<String>, cancelled: bool },
    Error(String),
}

/// Профиль считается рабочим, если открывается хотя бы половина целей.
pub fn healthy(s: &TestScore) -> bool {
    s.total > 0 && s.ok * 2 >= s.total
}

/// Есть ли вообще интернет (без него переключать профили бессмысленно).
/// Проверяем TCP-соединение с публичными DNS — их не блокируют.
pub fn internet_available() -> bool {
    use std::net::{SocketAddr, TcpStream};
    ["77.88.8.8:53", "1.1.1.1:53", "8.8.8.8:53"]
        .iter()
        .filter_map(|a| a.parse::<SocketAddr>().ok())
        .any(|a| TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok())
}

/// Фоновая проверка текущего подключения (до 20 КБ на цель — экономно по трафику).
pub fn health_check() -> TestScore {
    check_all_limited(&targets(), 20 * 1024)
}

/// Лучший профиль: больше всего успешных проверок, при равенстве — быстрее.
pub fn better(a: &TestScore, b: &TestScore) -> bool {
    a.ok > b.ok || (a.ok == b.ok && a.ok > 0 && a.avg_ms < b.avg_ms)
}

/// Режимы подбора: полный (все профили, лучший по результату) или
/// аварийный (профили по убыванию прошлых результатов, до первого рабочего).
pub fn run_auto_select(
    profiles: Vec<Profile>,
    game: GameFilter,
    cancel: Arc<AtomicBool>,
    stop_at_first_healthy: bool,
    mut emit: impl FnMut(TestEvent),
) {
    let targets = targets();
    if stop_at_first_healthy && !internet_available() {
        emit(TestEvent::Error("Нет подключения к интернету".into()));
        emit(TestEvent::Done { best: None, cancelled: true });
        return;
    }
    zapret::stop();
    let mut best: Option<(String, TestScore)> = None;
    let total = profiles.len();
    for (i, p) in profiles.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            zapret::stop();
            emit(TestEvent::Done { best: best.map(|b| b.0), cancelled: true });
            return;
        }
        emit(TestEvent::Progress { index: i, total, profile: p.id.clone() });
        let score = match zapret::start(p, game) {
            Ok(()) => {
                std::thread::sleep(Duration::from_millis(1200));
                let s = check_all(&targets);
                zapret::stop();
                s
            }
            Err(e) => {
                zapret::stop();
                if i == 0 && e.contains("служба zapret") {
                    emit(TestEvent::Error(e));
                    emit(TestEvent::Done { best: None, cancelled: true });
                    return;
                }
                TestScore { ok: 0, total: targets.len(), avg_ms: 0 }
            }
        };
        if best.as_ref().map(|(_, b)| better(&score, b)).unwrap_or(score.ok > 0) {
            best = Some((p.id.clone(), score.clone()));
        }
        emit(TestEvent::Result { profile: p.id.clone(), score: score.clone() });
        // все цели доступны и быстро — дальше можно не искать
        if score.ok == score.total && score.total > 0 && score.avg_ms < 600 {
            break;
        }
        if stop_at_first_healthy && healthy(&score) {
            break;
        }
    }
    emit(TestEvent::Done { best: best.map(|b| b.0), cancelled: false });
}
