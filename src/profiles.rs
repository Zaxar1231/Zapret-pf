//! Профили (стратегии) zapret — это файлы `general*.bat` из flowseal/zapret-discord-youtube.
//! Каждый .bat запускает `bin\winws.exe` с длинным списком аргументов. Мы разбираем
//! этот список и запускаем winws.exe напрямую — без консольных окон и без .bat.

use std::path::Path;

use crate::config::GameFilter;
use crate::paths;

#[derive(Clone, Debug)]
pub struct Profile {
    /// Имя файла без расширения, например `general (ALT3)` — служит ключом.
    pub id: String,
    /// Короткое имя для интерфейса, например `ALT3`.
    pub title: String,
    /// Аргументы winws.exe с ещё не подставленными переменными (%BIN%, %LISTS%, ...).
    pub args: Vec<String>,
}

impl Profile {
    /// Готовые аргументы для запуска winws.exe.
    pub fn resolved_args(&self, game: GameFilter) -> Vec<String> {
        let zapret = paths::with_trailing_sep(&paths::zapret_dir());
        let bin = paths::with_trailing_sep(&paths::bin_dir());
        let lists = paths::with_trailing_sep(&paths::lists_dir());
        let (gtcp, gudp) = game.ports();
        self.args
            .iter()
            .map(|a| {
                a.replace("%BIN%", &bin)
                    .replace("%LISTS%", &lists)
                    .replace("%~dp0", &zapret)
                    .replace("%GameFilterTCP%", gtcp)
                    .replace("%GameFilterUDP%", gudp)
                    .replace("%GameFilter%", gtcp)
                    .replace("%%", "%")
            })
            .collect()
    }
}

/// Загружает все профили из папки zapret, отсортированные как в service.bat
/// (числа сравниваются по значению: ALT2 < ALT10).
pub fn load_profiles() -> Vec<Profile> {
    let dir = paths::zapret_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Profile> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map(|e| e.eq_ignore_ascii_case("bat")).unwrap_or(false)
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().starts_with("service"))
                    .unwrap_or(true)
        })
        .filter_map(|p| parse_profile(&p))
        .collect();
    out.sort_by_key(|p| natural_key(&p.id));
    out
}

fn parse_profile(path: &Path) -> Option<Profile> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let args = parse_bat_args(&text)?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    Some(Profile { title: short_title(&id), id, args })
}

pub fn short_title(id: &str) -> String {
    let s = id.trim();
    if let Some(inner) = s
        .strip_prefix("general")
        .map(str::trim)
        .and_then(|r| r.strip_prefix('('))
        .and_then(|r| r.strip_suffix(')'))
    {
        return inner.trim().to_string();
    }
    if s.eq_ignore_ascii_case("general") {
        return "GENERAL".to_string();
    }
    s.to_string()
}

fn natural_key(s: &str) -> String {
    let mut out = String::new();
    let mut num = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            if !num.is_empty() {
                out.push_str(&format!("{:0>8}", num));
                num.clear();
            }
            out.push(c);
        }
    }
    if !num.is_empty() {
        out.push_str(&format!("{:0>8}", num));
    }
    out
}

/// Извлекает аргументы winws.exe из текста .bat.
pub fn parse_bat_args(text: &str) -> Option<Vec<String>> {
    // 1. Склеиваем строки, продолженные символом ^ в конце.
    let mut logical = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        let t = line.trim_end();
        if let Some(stripped) = t.strip_suffix('^') {
            cur.push_str(stripped);
            cur.push(' ');
        } else {
            cur.push_str(t);
            logical.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        logical.push(cur);
    }

    // 2. Ищем строку с запуском winws.exe.
    for line in logical {
        let l = line.trim_start();
        if l.starts_with("::") || l.to_lowercase().starts_with("rem ") {
            continue;
        }
        let lower = line.to_lowercase();
        if let Some(pos) = lower.find("winws.exe") {
            let rest = &line[pos + "winws.exe".len()..];
            let rest = rest.strip_prefix('"').unwrap_or(rest);
            let args = tokenize(rest);
            if !args.is_empty() {
                return Some(args);
            }
        }
    }
    None
}

/// Разбивает командную строку cmd.exe на аргументы: пробелы вне кавычек разделяют,
/// кавычки удаляются, `^x` вне кавычек превращается в `x`.
fn tokenize(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut has = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_q = !in_q;
                has = true;
            }
            '^' if !in_q => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    has = true;
                }
            }
            c if c.is_whitespace() && !in_q => {
                if has {
                    args.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        args.push(cur);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_general_bat() {
        let bat = r#"@echo off
cd /d "%~dp0"
set "BIN=%~dp0bin\"
start "zapret: %~n0" /min "%BIN%winws.exe" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443 ^
--filter-udp=443 --hostlist="%LISTS%list-general.txt" --dpi-desync=fake --new ^
--filter-tcp=443 --dpi-desync-fake-tls=0x00 --dpi-desync-fake-tls="%BIN%tls clienthello.bin"
"#;
        let args = parse_bat_args(bat).unwrap();
        assert_eq!(args[0], "--wf-tcp=80,443,%GameFilterTCP%");
        assert!(args.contains(&"--hostlist=%LISTS%list-general.txt".to_string()));
        assert!(args.contains(&"--new".to_string()));
        assert_eq!(args.last().unwrap(), "--dpi-desync-fake-tls=%BIN%tls clienthello.bin");
    }

    #[test]
    fn titles() {
        assert_eq!(short_title("general (ALT10)"), "ALT10");
        assert_eq!(short_title("general"), "GENERAL");
        assert_eq!(short_title("general (FAKE TLS AUTO)"), "FAKE TLS AUTO");
    }

    #[test]
    fn natural_sort() {
        assert!(natural_key("general (ALT2)") < natural_key("general (ALT10)"));
    }
}
