//! Обновления: zapret (flowseal/zapret-discord-youtube) и самого приложения.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::paths;
use crate::sys;
use crate::zapret;

pub const ZAPRET_REPO: &str = "Flowseal/zapret-discord-youtube";
pub const APP_REPO: &str = "zaxar1231/zapret-pf";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const ZAPRET_VERSION_URL: &str =
    "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt";
const ZAPRET_MAIN_ZIP: &str = "https://github.com/Flowseal/zapret-discord-youtube/archive/refs/heads/main.zip";

fn client(timeout: u64) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(8))
        .user_agent(concat!("ZapretPF/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("http client")
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize, Clone)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn latest_release(repo: &str) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let r = client(15)
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("GitHub ответил {}", r.status()));
    }
    r.json::<Release>().map_err(|e| e.to_string())
}

/// Сравнение версий вида `1.10.4` / `v1.2.0`.
pub fn is_newer(remote: &str, local: &str) -> bool {
    fn parse(s: &str) -> Vec<u64> {
        s.trim()
            .trim_start_matches(['v', 'V'])
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty())
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }
    let (r, l) = (parse(remote), parse(local));
    for i in 0..r.len().max(l.len()) {
        let (a, b) = (r.get(i).copied().unwrap_or(0), l.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

// ---------------------------------------------------------------- zapret

pub fn zapret_remote_version() -> Result<String, String> {
    let r = client(10)
        .get(ZAPRET_VERSION_URL)
        .header("Cache-Control", "no-cache")
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    let v = r.text().map_err(|e| e.to_string())?.trim().to_string();
    if v.is_empty() || v.len() > 32 {
        return Err("неверный ответ сервера".into());
    }
    Ok(v)
}

fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(String)) -> Result<(), String> {
    let mut r = client(600).get(url).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {} — {url}", r.status()));
    }
    let total = r.content_length();
    let mut f = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut last = 0u64;
    loop {
        let n = r.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut f, &buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last > 256 * 1024 {
            last = done;
            progress(match total {
                Some(t) if t > 0 => format!("Загрузка… {}%", done * 100 / t),
                _ => format!("Загрузка… {:.1} МБ", done as f64 / 1_048_576.0),
            });
        }
    }
    Ok(())
}

fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|e| format!("архив повреждён: {e}"))?;
    for i in 0..ar.len() {
        let mut entry = ar.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut o = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut o).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Ищет в распакованном архиве папку, содержащую `bin\winws.exe`.
fn find_zapret_root(dir: &Path, depth: u32) -> Option<PathBuf> {
    if dir.join("bin").join("winws.exe").exists() {
        return Some(dir.to_path_buf());
    }
    if depth == 0 {
        return None;
    }
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        if e.path().is_dir() {
            if let Some(r) = find_zapret_root(&e.path(), depth - 1) {
                return Some(r);
            }
        }
    }
    None
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            std::fs::copy(e.path(), to)?;
        }
    }
    Ok(())
}

/// Скачивает и устанавливает последнюю версию zapret в `<install>\zapret`.
/// Пользовательские списки (`lists\*-user.txt`) сохраняются.
pub fn update_zapret(progress: &mut dyn FnMut(String)) -> Result<String, String> {
    progress("Поиск релиза…".into());
    let url = match latest_release(ZAPRET_REPO) {
        Ok(rel) => rel
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(".zip"))
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_else(|| ZAPRET_MAIN_ZIP.to_string()),
        Err(_) => ZAPRET_MAIN_ZIP.to_string(),
    };

    let zip_path = paths::tmp_dir().join("zapret.zip");
    download(&url, &zip_path, progress)?;
    let r = install_zapret_zip(&zip_path, progress);
    let _ = std::fs::remove_file(&zip_path);
    r
}

/// Устанавливает zapret из zip-архива (релиз или архив ветки GitHub).
pub fn install_zapret_zip(zip_path: &Path, progress: &mut dyn FnMut(String)) -> Result<String, String> {
    let ex = paths::tmp_dir().join("zapret-extract");
    let _ = std::fs::remove_dir_all(&ex);

    progress("Распаковка…".into());
    extract_zip(&zip_path, &ex)?;
    let root = find_zapret_root(&ex, 3).ok_or("в архиве нет bin\\winws.exe")?;

    progress("Установка…".into());
    zapret::stop();
    sys::stop_windivert();

    let target = paths::zapret_dir();
    let old = paths::install_dir().join("zapret.old");
    let _ = std::fs::remove_dir_all(&old);

    // сохраняем пользовательские файлы
    let mut keep: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(target.join("lists")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_lowercase();
            if n.ends_with("-user.txt") {
                if let Ok(b) = std::fs::read(e.path()) {
                    keep.push((PathBuf::from("lists").join(e.file_name()), b));
                }
            }
        }
    }

    if target.exists() {
        std::fs::rename(&target, &old).map_err(|e| format!("не удалось заменить папку zapret: {e}"))?;
    }
    if let Err(e) = std::fs::rename(&root, &target).or_else(|_| copy_dir(&root, &target)) {
        let _ = std::fs::remove_dir_all(&target);
        let _ = std::fs::rename(&old, &target);
        return Err(format!("ошибка установки: {e}"));
    }
    for (rel, bytes) in keep {
        let _ = std::fs::write(target.join(rel), bytes);
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&ex);
    zapret::ensure_user_lists();
    Ok(zapret::local_version().unwrap_or_else(|| "?".into()))
}

/// Обновить список IPSet (`lists\ipset-all.txt`), как пункт меню service.bat.
pub fn update_ipset() -> Result<(), String> {
    let url = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt";
    let r = client(30).get(url).send().map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    let body = r.bytes().map_err(|e| e.to_string())?;
    std::fs::write(paths::lists_dir().join("ipset-all.txt"), &body).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- app

#[derive(Clone, Debug)]
pub struct AppUpdate {
    pub version: String,
    pub url: String,
    pub page: String,
}

pub fn check_app_update() -> Result<Option<AppUpdate>, String> {
    let rel = latest_release(APP_REPO)?;
    if !is_newer(&rel.tag_name, APP_VERSION) {
        return Ok(None);
    }
    let asset = rel
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_lowercase();
            n.ends_with(".exe") && n.contains("setup")
        })
        .cloned();
    Ok(asset.map(|a| AppUpdate {
        version: rel.tag_name.trim_start_matches(['v', 'V']).to_string(),
        url: a.browser_download_url,
        page: rel.html_url,
    }))
}

/// Скачивает установщик новой версии и запускает его в тихом режиме.
/// Установщик сам закроет приложение, обновит файлы и запустит его снова.
pub fn install_app_update(upd: &AppUpdate, progress: &mut dyn FnMut(String)) -> Result<(), String> {
    let dest = std::env::temp_dir().join(format!("ZapretPF-Setup-{}.exe", upd.version));
    download(&upd.url, &dest, progress)?;
    progress("Запуск установщика…".into());
    // Установщик берёт папку установки из реестра (InstallDirRegKey).
    std::process::Command::new(&dest)
        .args(["/S", "/UPDATE"])
        .spawn()
        .map_err(|e| format!("не удалось запустить установщик: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_newer;
    #[test]
    fn versions() {
        assert!(is_newer("1.10.5", "1.10.4"));
        assert!(is_newer("v1.11.0", "1.10.9"));
        assert!(!is_newer("1.10.4", "1.10.4"));
        assert!(!is_newer("1.9", "1.10.0"));
        assert!(is_newer("1.10.4", "1.10"));
    }
}

#[cfg(test)]
mod install_tests {
    /// Собирает zip в формате GitHub-архива из локальной копии репозитория zapret
    /// (путь в ZAPRET_SRC) и проверяет установку и сохранение пользовательских списков.
    #[test]
    #[ignore]
    fn install_from_local_zip() {
        let src = std::path::PathBuf::from(std::env::var("ZAPRET_SRC").expect("ZAPRET_SRC"));
        let zip_path = crate::paths::tmp_dir().join("test.zip");
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        fn add(w: &mut zip::ZipWriter<std::fs::File>, base: &std::path::Path, dir: &std::path::Path, o: zip::write::SimpleFileOptions) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.file_name().unwrap() == ".git" { continue; }
                let rel = format!("zapret-discord-youtube-main/{}", p.strip_prefix(base).unwrap().to_string_lossy());
                if p.is_dir() { w.add_directory(rel, o).unwrap(); add(w, base, &p, o); }
                else { w.start_file(rel, o).unwrap(); std::io::Write::write_all(w, &std::fs::read(&p).unwrap()).unwrap(); }
            }
        }
        add(&mut w, &src, &src, opts);
        w.finish().unwrap();

        // пользовательский список должен пережить обновление
        let lists = crate::paths::lists_dir();
        std::fs::create_dir_all(&lists).unwrap();
        std::fs::write(lists.join("list-general-user.txt"), "my.domain\n").unwrap();

        let v = super::install_zapret_zip(&zip_path, &mut |s| eprintln!("{s}")).unwrap();
        eprintln!("installed {v}");
        assert!(crate::paths::zapret_installed());
        assert_eq!(std::fs::read_to_string(lists.join("list-general-user.txt")).unwrap(), "my.domain\n");
        let profiles = crate::profiles::load_profiles();
        assert!(profiles.len() > 5, "profiles: {}", profiles.len());
        eprintln!("{:?}", profiles[0].resolved_args(crate::config::GameFilter::All));
        for p in &profiles {
            let args = p.resolved_args(crate::config::GameFilter::Off);
            assert!(args.iter().all(|a| !a.contains('%')), "{}: {:?}", p.id, args);
            assert!(args.iter().any(|a| a == "--new"));
        }
    }
}
