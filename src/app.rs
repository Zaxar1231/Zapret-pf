//! Интерфейс приложения: главный экран «как у VPN» и экран настроек.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{pos2, vec2, Align, Align2, Color32, CornerRadius, Layout, Rect, Sense, Stroke, StrokeKind, Theme, Ui};

use crate::config::{Config, GameFilter, TestScore, ThemeMode};
use crate::profiles::{self, Profile};
use crate::theme::{self, Palette};
use crate::tester::{self, TestEvent};
use crate::updater::{self, AppUpdate};
use crate::widgets::{self, ButtonKind, Icon};
use crate::{paths, sys, zapret};

enum Msg {
    Running(bool),
    Connected(Result<String, String>),
    Disconnected,
    Test(TestEvent),
    ZapretRemote(Result<String, String>),
    AppUpdate(Result<Option<AppUpdate>, String>, bool),
    Progress(String),
    ZapretUpdated(Result<String, String>, bool),
    AppUpdateStarted(Result<(), String>),
    Info(String),
    /// Результат фоновой проверки: (оценка, есть ли интернет).
    Health(TestScore, bool),
}

#[derive(Clone, PartialEq)]
enum Busy {
    None,
    Connecting,
    Disconnecting,
    /// `auto` — аварийное переключение (текущий профиль перестал работать).
    Testing { index: usize, total: usize, profile: String, auto: bool },
    UpdatingZapret(String),
    UpdatingApp(String),
}

#[derive(PartialEq)]
enum Screen {
    Main,
    Settings,
}

pub struct ZapretApp {
    cfg: Config,
    profiles: Vec<Profile>,
    connected: bool,
    busy: Busy,
    screen: Screen,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    cancel: Arc<AtomicBool>,
    pause_poll: Arc<AtomicBool>,
    zapret_local: Option<String>,
    zapret_remote: Option<String>,
    zapret_check_error: Option<String>,
    app_update: Option<AppUpdate>,
    app_checked: bool,
    service_conflict: bool,
    snackbar: Option<(String, Instant)>,
    connected_since: Option<Instant>,
    minimize_once: bool,
    // автопереключение
    last_health: Instant,
    health_running: bool,
    health_failures: u32,
    crash_times: Vec<Instant>,
    failover_from: Option<String>,
}

/// Как часто проверять, что текущий профиль ещё работает.
const HEALTH_INTERVAL: Duration = Duration::from_secs(180);

impl ZapretApp {
    pub fn new(cc: &eframe::CreationContext<'_>, autostart: bool, instance: Option<std::net::TcpListener>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let cfg = Config::load();
        apply_theme_pref(&cc.egui_ctx, cfg.theme);

        let (tx, rx) = channel();
        let mut app = Self {
            profiles: profiles::load_profiles(),
            connected: zapret::is_running(),
            busy: Busy::None,
            screen: Screen::Main,
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            pause_poll: Arc::new(AtomicBool::new(false)),
            zapret_local: zapret::local_version(),
            zapret_remote: None,
            zapret_check_error: None,
            app_update: None,
            app_checked: false,
            service_conflict: sys::service_exists("zapret"),
            snackbar: None,
            connected_since: None,
            minimize_once: autostart,
            last_health: Instant::now(),
            health_running: false,
            health_failures: 0,
            crash_times: Vec::new(),
            failover_from: None,
            cfg,
        };
        if app.connected {
            app.connected_since = Some(Instant::now());
        }
        if app.cfg.selected_profile.as_ref().map(|s| !app.profiles.iter().any(|p| &p.id == s)).unwrap_or(true) {
            app.cfg.selected_profile = app.best_scored().or_else(|| app.profiles.first().map(|p| p.id.clone()));
        }
        app.cfg.autostart = sys::autostart_enabled();

        app.spawn_status_poll();
        if let Some(l) = instance {
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                for _ in l.incoming().flatten() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    ctx.request_repaint();
                }
            });
        }
        if app.cfg.check_updates {
            app.check_updates(false);
        }
        if !paths::zapret_installed() {
            app.notify("Файлы zapret не найдены — нажмите «Скачать zapret»");
        } else if app.cfg.autoconnect && !app.connected && !app.service_conflict {
            app.connect();
        }
        app
    }

    // ------------------------------------------------------------ helpers

    fn send(tx: &Sender<Msg>, ctx: &egui::Context, m: Msg) {
        let _ = tx.send(m);
        ctx.request_repaint();
    }

    fn bg(&self, f: impl FnOnce(Sender<Msg>, egui::Context) + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || f(tx, ctx));
    }

    fn notify(&mut self, s: impl Into<String>) {
        self.snackbar = Some((s.into(), Instant::now()));
    }

    fn selected(&self) -> Option<&Profile> {
        let id = self.cfg.selected_profile.as_ref()?;
        self.profiles.iter().find(|p| &p.id == id)
    }

    fn best_scored(&self) -> Option<String> {
        let mut best: Option<(&String, &TestScore)> = None;
        for (k, v) in &self.cfg.scores {
            if !self.profiles.iter().any(|p| &p.id == k) {
                continue;
            }
            if best.map(|(_, b)| tester::better(v, b)).unwrap_or(v.ok > 0) {
                best = Some((k, v));
            }
        }
        best.map(|(k, _)| k.clone())
    }

    fn is_busy(&self) -> bool {
        self.busy != Busy::None
    }

    fn spawn_status_poll(&self) {
        let pause = self.pause_poll.clone();
        self.bg(move |tx, ctx| {
            let mut last: Option<bool> = None;
            loop {
                std::thread::sleep(Duration::from_secs(3));
                if pause.load(Ordering::Relaxed) {
                    last = None;
                    continue;
                }
                let r = zapret::is_running();
                if last != Some(r) {
                    last = Some(r);
                    Self::send(&tx, &ctx, Msg::Running(r));
                }
            }
        });
    }

    // ------------------------------------------------------------ actions

    fn connect(&mut self) {
        let Some(p) = self.selected().cloned() else {
            self.notify("Выберите профиль");
            return;
        };
        self.busy = Busy::Connecting;
        self.pause_poll.store(true, Ordering::Relaxed);
        let game = self.cfg.game_filter;
        self.bg(move |tx, ctx| {
            let r = zapret::start(&p, game).map(|_| p.id.clone());
            Self::send(&tx, &ctx, Msg::Connected(r));
        });
    }

    fn disconnect(&mut self) {
        self.busy = Busy::Disconnecting;
        self.pause_poll.store(true, Ordering::Relaxed);
        self.bg(move |tx, ctx| {
            zapret::stop();
            Self::send(&tx, &ctx, Msg::Disconnected);
        });
    }

    fn toggle(&mut self) {
        match self.busy {
            Busy::Testing { .. } => {
                self.cancel.store(true, Ordering::Relaxed);
            }
            Busy::None => {
                if self.connected {
                    self.disconnect()
                } else {
                    self.connect()
                }
            }
            _ => {}
        }
    }

    fn select_profile(&mut self, id: String) {
        if self.cfg.selected_profile.as_deref() == Some(&id) {
            return;
        }
        self.cfg.selected_profile = Some(id);
        self.cfg.save();
        if self.connected && !self.is_busy() {
            self.connect();
        }
    }

    /// `failover = false` — полный подбор по кнопке;
    /// `failover = true` — текущий профиль перестал работать: перебираем остальные
    /// (сначала с лучшими прошлыми результатами) до первого рабочего.
    fn auto_select(&mut self, failover: bool) {
        if self.profiles.is_empty() {
            self.notify("Нет профилей для проверки");
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.pause_poll.store(true, Ordering::Relaxed);
        self.health_failures = 0;
        let mut profiles = self.profiles.clone();
        if failover {
            let cur = self.cfg.selected_profile.clone();
            self.failover_from = cur.clone();
            let scores = self.cfg.scores.clone();
            let key = |p: &Profile| {
                let s = scores.get(&p.id).cloned().unwrap_or_default();
                // неизвестные профили — посередине, текущий — в самом конце
                let current = cur.as_deref() == Some(p.id.as_str());
                (current, std::cmp::Reverse(s.ok), s.avg_ms)
            };
            profiles.sort_by_key(key);
        } else {
            self.failover_from = None;
            self.cfg.scores.clear();
        }
        self.busy = Busy::Testing { index: 0, total: profiles.len(), profile: String::new(), auto: failover };
        let game = self.cfg.game_filter;
        let cancel = self.cancel.clone();
        self.bg(move |tx, ctx| {
            tester::run_auto_select(profiles, game, cancel, failover, |ev| Self::send(&tx, &ctx, Msg::Test(ev)));
        });
    }

    /// Периодическая проверка: работает ли ещё текущий профиль.
    fn maybe_health_check(&mut self) {
        if !self.cfg.auto_switch || !self.connected || self.is_busy() || self.health_running {
            return;
        }
        if self.last_health.elapsed() < HEALTH_INTERVAL {
            return;
        }
        self.health_running = true;
        self.bg(|tx, ctx| {
            let score = tester::health_check();
            let net = tester::healthy(&score) || tester::internet_available();
            Self::send(&tx, &ctx, Msg::Health(score, net));
        });
    }

    fn check_updates(&mut self, manual: bool) {
        self.zapret_check_error = None;
        self.bg(move |tx, ctx| {
            Self::send(&tx, &ctx, Msg::ZapretRemote(updater::zapret_remote_version()));
        });
        self.bg(move |tx, ctx| {
            Self::send(&tx, &ctx, Msg::AppUpdate(updater::check_app_update(), manual));
        });
    }

    fn update_zapret(&mut self) {
        let was_connected = self.connected;
        self.busy = Busy::UpdatingZapret("Подготовка…".into());
        self.pause_poll.store(true, Ordering::Relaxed);
        self.bg(move |tx, ctx| {
            let r = updater::update_zapret(&mut |s| Self::send(&tx, &ctx, Msg::Progress(s)));
            Self::send(&tx, &ctx, Msg::ZapretUpdated(r, was_connected));
        });
    }

    fn update_app(&mut self) {
        let Some(u) = self.app_update.clone() else { return };
        self.busy = Busy::UpdatingApp("Подготовка…".into());
        self.bg(move |tx, ctx| {
            let r = updater::install_app_update(&u, &mut |s| Self::send(&tx, &ctx, Msg::Progress(s)));
            Self::send(&tx, &ctx, Msg::AppUpdateStarted(r));
        });
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Running(r) => {
                    if !self.is_busy() && r != self.connected {
                        let crashed = self.connected && !r;
                        self.connected = r;
                        self.connected_since = r.then(Instant::now);
                        if crashed && self.cfg.auto_switch {
                            // winws.exe завершился не по нашей команде
                            self.crash_times.retain(|t| t.elapsed() < Duration::from_secs(600));
                            self.crash_times.push(Instant::now());
                            if self.crash_times.len() <= 2 {
                                self.notify("winws.exe неожиданно завершился — перезапускаю");
                                self.connect();
                            } else {
                                self.crash_times.clear();
                                self.notify("Профиль постоянно падает — ищу другой");
                                self.auto_select(true);
                            }
                        }
                    }
                }
                Msg::Health(score, net) => {
                    self.health_running = false;
                    self.last_health = Instant::now();
                    if !self.connected || self.is_busy() {
                        continue;
                    }
                    if tester::healthy(&score) {
                        self.health_failures = 0;
                        if let Some(id) = self.cfg.selected_profile.clone() {
                            self.cfg.scores.insert(id, score);
                        }
                    } else if net {
                        self.health_failures += 1;
                        if self.health_failures >= 2 {
                            self.notify("Текущий профиль перестал работать — ищу другой");
                            self.auto_select(true);
                        } else {
                            // перепроверим через минуту, чтобы не реагировать на случайный сбой
                            self.last_health = Instant::now() - HEALTH_INTERVAL + Duration::from_secs(60);
                        }
                    }
                }
                Msg::Connected(r) => {
                    self.busy = Busy::None;
                    self.pause_poll.store(false, Ordering::Relaxed);
                    match r {
                        Ok(_) => {
                            self.connected = true;
                            self.connected_since = Some(Instant::now());
                            // первая проверка — примерно через минуту после подключения
                            self.last_health = Instant::now() - HEALTH_INTERVAL + Duration::from_secs(60);
                            self.health_failures = 0;
                        }
                        Err(e) => {
                            self.connected = false;
                            self.connected_since = None;
                            self.service_conflict = sys::service_exists("zapret");
                            self.notify(e);
                        }
                    }
                }
                Msg::Disconnected => {
                    self.busy = Busy::None;
                    self.pause_poll.store(false, Ordering::Relaxed);
                    self.connected = false;
                    self.connected_since = None;
                }
                Msg::Test(ev) => match ev {
                    TestEvent::Progress { index, total, profile } => {
                        let auto = matches!(self.busy, Busy::Testing { auto: true, .. });
                        self.busy = Busy::Testing { index, total, profile, auto };
                    }
                    TestEvent::Result { profile, score } => {
                        self.cfg.scores.insert(profile, score);
                    }
                    TestEvent::Error(e) => self.notify(e),
                    TestEvent::Done { best, cancelled } => {
                        let failover = matches!(self.busy, Busy::Testing { auto: true, .. });
                        self.busy = Busy::None;
                        self.pause_poll.store(false, Ordering::Relaxed);
                        self.connected = false;
                        self.connected_since = None;
                        let from = self.failover_from.take();
                        match best {
                            Some(b) if failover => {
                                let title = profiles::short_title(&b);
                                let old = from.as_deref().map(profiles::short_title).unwrap_or_default();
                                self.cfg.selected_profile = Some(b);
                                let from = from.clone();
                                if from.as_deref() == self.cfg.selected_profile.as_deref() {
                                    self.notify(format!("Профиль {title} снова работает"));
                                } else {
                                    self.notify(format!("Профиль {old} перестал работать — переключено на {title}"));
                                }
                                self.connect();
                            }
                            None if failover => {
                                // рабочего нет (или пропал интернет) — возвращаем прежний и ждём
                                self.cfg.selected_profile = from.or(self.cfg.selected_profile.clone());
                                if !cancelled {
                                    self.notify("Рабочий профиль не найден, повторю проверку позже");
                                }
                                self.connect();
                            }
                            Some(b) => {
                                let title = profiles::short_title(&b);
                                self.cfg.selected_profile = Some(b);
                                self.notify(if cancelled {
                                    format!("Подбор остановлен. Лучший из проверенных: {title}")
                                } else {
                                    format!("Лучший профиль: {title}")
                                });
                                self.connect();
                            }
                            None if !cancelled => {
                                self.notify("Ни один профиль не открыл сайты. Попробуйте обновить zapret или включить игровой фильтр.")
                            }
                            None => self.notify("Подбор остановлен"),
                        }
                        self.cfg.save();
                    }
                },
                Msg::ZapretRemote(r) => match r {
                    Ok(v) => self.zapret_remote = Some(v),
                    Err(e) => self.zapret_check_error = Some(e),
                },
                Msg::AppUpdate(r, manual) => {
                    self.app_checked = true;
                    match r {
                        Ok(u) => {
                            if manual && u.is_none() {
                                self.notify(format!("У вас последняя версия приложения ({})", updater::APP_VERSION));
                            }
                            self.app_update = u;
                        }
                        Err(e) if manual => self.notify(format!("Не удалось проверить обновления: {e}")),
                        Err(_) => {}
                    }
                }
                Msg::Progress(s) => match &mut self.busy {
                    Busy::UpdatingZapret(p) | Busy::UpdatingApp(p) => *p = s,
                    _ => {}
                },
                Msg::ZapretUpdated(r, reconnect) => {
                    self.busy = Busy::None;
                    self.pause_poll.store(false, Ordering::Relaxed);
                    self.connected = zapret::is_running();
                    self.profiles = profiles::load_profiles();
                    self.zapret_local = zapret::local_version();
                    match r {
                        Ok(v) => {
                            self.notify(format!("zapret обновлён до версии {v}"));
                            if self.selected().is_none() {
                                self.cfg.selected_profile = self.profiles.first().map(|p| p.id.clone());
                            }
                            if reconnect || (self.cfg.autoconnect && !self.connected) {
                                self.connect();
                            }
                        }
                        Err(e) => self.notify(format!("Ошибка обновления zapret: {e}")),
                    }
                }
                Msg::AppUpdateStarted(r) => match r {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => {
                        self.busy = Busy::None;
                        self.notify(format!("Ошибка обновления: {e}"));
                    }
                },
                Msg::Info(s) => self.notify(s),
            }
        }
    }

    // ------------------------------------------------------------ UI

    fn top_bar(&mut self, ui: &mut Ui, pal: &Palette) {
        ui.horizontal(|ui| {
            ui.set_height(56.0);
            ui.add_space(4.0);
            if self.screen == Screen::Settings {
                if widgets::icon_button(ui, pal, Icon::Back, "Назад").clicked() {
                    self.screen = Screen::Main;
                }
                ui.label(egui::RichText::new("Настройки").font(theme::regular(22.0)).color(pal.on_surface));
            } else {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Zapret PF").font(theme::semibold(22.0)).color(pal.on_surface));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(4.0);
                if self.screen == Screen::Main {
                    if widgets::icon_button(ui, pal, Icon::Gear, "Настройки").clicked() {
                        self.screen = Screen::Settings;
                    }
                    if widgets::icon_button(ui, pal, Icon::Refresh, "Проверить обновления").clicked() {
                        self.check_updates(true);
                    }
                }
            });
        });
    }

    fn banner(&mut self, ui: &mut Ui, pal: &Palette, icon: Icon, bg: Color32, fg: Color32, title: &str, body: &str, action: &str) -> bool {
        let mut clicked = false;
        egui::Frame::new()
            .fill(bg)
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin { left: 16, right: 8, top: 12, bottom: 8 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                    widgets::paint_icon(ui, icon, r, fg);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(egui::RichText::new(title).font(theme::semibold(14.0)).color(fg));
                        if !body.is_empty() {
                            ui.label(egui::RichText::new(body).font(theme::regular(13.0)).color(fg.gamma_multiply(0.85)));
                        }
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let enabled = !self.is_busy();
                    if widgets::button(ui, pal, ButtonKind::Text, None, action, enabled).clicked() {
                        clicked = true;
                    }
                });
            });
        ui.add_space(8.0);
        clicked
    }

    fn banners(&mut self, ui: &mut Ui, pal: &Palette) {
        // Ход обновления
        if let Busy::UpdatingZapret(s) | Busy::UpdatingApp(s) = self.busy.clone() {
            egui::Frame::new()
                .fill(pal.surface_container_high)
                .corner_radius(CornerRadius::same(16))
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let what = if matches!(self.busy, Busy::UpdatingApp(_)) { "Обновление приложения" } else { "Обновление zapret" };
                    ui.label(egui::RichText::new(what).font(theme::semibold(14.0)).color(pal.on_surface));
                    ui.label(egui::RichText::new(s).font(theme::regular(13.0)).color(pal.on_surface_variant));
                    widgets::linear_progress(ui, pal, None);
                });
            ui.add_space(8.0);
            return;
        }

        if self.service_conflict {
            if self.banner(
                ui,
                pal,
                Icon::Warning,
                pal.error_container,
                pal.on_error_container,
                "Найдена служба zapret",
                "Служба из service.bat мешает работе приложения.",
                "Удалить",
            ) {
                self.pause_poll.store(true, Ordering::Relaxed);
                let pause = self.pause_poll.clone();
                self.bg(move |tx, ctx| {
                    sys::remove_zapret_service();
                    pause.store(false, Ordering::Relaxed);
                    Self::send(&tx, &ctx, Msg::Info("Служба zapret удалена".into()));
                });
                self.service_conflict = false;
            }
        }

        if !paths::zapret_installed() {
            if self.banner(
                ui,
                pal,
                Icon::Download,
                pal.tertiary_container,
                pal.on_tertiary_container,
                "zapret не установлен",
                "Скачать последнюю версию flowseal/zapret-discord-youtube",
                "Скачать",
            ) {
                self.update_zapret();
            }
        } else if let (Some(remote), local) = (self.zapret_remote.clone(), self.zapret_local.clone()) {
            if local.as_deref().map(|l| updater::is_newer(&remote, l)).unwrap_or(true) {
                let body = format!("{} → {}", local.unwrap_or_else(|| "?".into()), remote);
                if self.banner(ui, pal, Icon::Download, pal.primary_container, pal.on_primary_container, "Доступно обновление zapret", &body, "Обновить") {
                    self.update_zapret();
                }
            }
        }

        if let Some(u) = self.app_update.clone() {
            if self.cfg.skipped_app_version.as_deref() != Some(&u.version) {
                let body = format!("Версия {} (у вас {})", u.version, updater::APP_VERSION);
                if self.banner(ui, pal, Icon::Info, pal.secondary_container, pal.on_secondary_container, "Новая версия Zapret PF", &body, "Установить") {
                    self.update_app();
                }
            }
        }
    }

    fn hero(&mut self, ui: &mut Ui, pal: &Palette) {
        let testing = matches!(self.busy, Busy::Testing { .. });
        let working = matches!(self.busy, Busy::Connecting | Busy::Disconnecting) || testing;
        let on = self.connected && !working;

        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            let size = 176.0;
            let (rect, resp) = ui.allocate_exact_size(vec2(size + 48.0, size + 48.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let c = rect.center();
            let t_on = ui.ctx().animate_bool_with_time(ui.scope_id().with("on"), on, 0.35);
            let time = ui.input(|i| i.time) as f32;
            let p = ui.painter();

            // ореол
            if on {
                let k = (time * 0.8).fract();
                p.circle_filled(c, size / 2.0 + 6.0 + 18.0 * k, pal.primary.gamma_multiply(0.18 * (1.0 - k)));
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
            p.circle_filled(c, size / 2.0 + 10.0, widgets::lerp_color(pal.surface_container, pal.primary_container, t_on));

            let fill = widgets::lerp_color(pal.surface_container_highest, pal.primary, t_on);
            p.circle_filled(c, size / 2.0, fill);
            if resp.hovered() {
                p.circle_filled(c, size / 2.0, pal.on_surface.gamma_multiply(0.06));
            }
            if resp.is_pointer_button_down_on() {
                p.circle_filled(c, size / 2.0, pal.on_surface.gamma_multiply(0.10));
            }

            // крутящаяся дуга при подключении / подборе
            if working {
                let start = time * 4.0;
                let len = 1.2 + 0.8 * (time * 2.0).sin();
                let pts: Vec<_> = (0..=40)
                    .map(|i| {
                        let a = start + len * i as f32 / 40.0;
                        pos2(c.x + (size / 2.0 + 10.0) * a.cos(), c.y + (size / 2.0 + 10.0) * a.sin())
                    })
                    .collect();
                p.add(egui::Shape::line(pts, Stroke::new(4.0, pal.primary)));
                ui.ctx().request_repaint();
            }

            let icon_col = widgets::lerp_color(pal.on_surface_variant, pal.on_primary, t_on);
            widgets::paint_icon(ui, Icon::Power, Rect::from_center_size(c, vec2(68.0, 68.0)), icon_col);
            if resp.clicked() {
                self.toggle();
            }

            ui.add_space(4.0);
            let (status, color) = match &self.busy {
                Busy::Connecting => ("Подключение…".to_string(), pal.on_surface_variant),
                Busy::Disconnecting => ("Отключение…".to_string(), pal.on_surface_variant),
                Busy::Testing { auto: true, .. } => ("Поиск рабочего профиля…".to_string(), pal.primary),
                Busy::Testing { .. } => ("Подбор профиля…".to_string(), pal.primary),
                _ if self.connected => ("Подключено".to_string(), pal.primary),
                _ => ("Не подключено".to_string(), pal.on_surface_variant),
            };
            ui.label(egui::RichText::new(status).font(theme::semibold(24.0)).color(color));

            let sub = match &self.busy {
                Busy::Testing { index, total, profile, .. } => {
                    format!("Проверка {} из {} · {}", index + 1, total, profiles::short_title(profile))
                }
                _ if self.connected => {
                    let name = self.selected().map(|p| p.title.clone()).unwrap_or_default();
                    match self.connected_since {
                        Some(t) => {
                            let s = t.elapsed().as_secs();
                            ui.ctx().request_repaint_after(Duration::from_secs(1));
                            format!("{name} · {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
                        }
                        None => name,
                    }
                }
                _ => match self.selected() {
                    Some(p) => format!("Профиль: {}", p.title),
                    None => "Профиль не выбран".into(),
                },
            };
            ui.label(egui::RichText::new(sub).font(theme::regular(14.0)).color(pal.on_surface_variant));

            if let Busy::Testing { index, total, .. } = self.busy {
                ui.add_space(4.0);
                ui.allocate_ui(vec2(240.0, 8.0), |ui| {
                    widgets::linear_progress(ui, pal, Some((index as f32 + 0.5) / total.max(1) as f32));
                });
            }

            ui.add_space(8.0);
            let label = if testing { "Остановить подбор" } else { "Автоподбор профиля" };
            let kind = if testing { ButtonKind::Outlined } else { ButtonKind::Tonal };
            let enabled = testing || (!self.is_busy() && paths::zapret_installed());
            if widgets::button(ui, pal, kind, Some(Icon::Sparkle), label, enabled).clicked() {
                if testing {
                    self.cancel.store(true, Ordering::Relaxed);
                } else {
                    self.auto_select(false);
                }
            }
        });
    }

    fn profile_list(&mut self, ui: &mut Ui, pal: &Palette) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Профили").font(theme::semibold(14.0)).color(pal.primary));
            ui.label(egui::RichText::new(format!("{}", self.profiles.len())).font(theme::regular(13.0)).color(pal.on_surface_variant));
        });
        ui.add_space(2.0);

        if self.profiles.is_empty() {
            ui.label(egui::RichText::new("Профили появятся после загрузки zapret.").color(pal.on_surface_variant));
            return;
        }

        let selected = self.cfg.selected_profile.clone();
        let testing_now = match &self.busy {
            Busy::Testing { profile, .. } => Some(profile.clone()),
            _ => None,
        };
        let best = self.best_scored();
        let mut clicked: Option<String> = None;
        let locked = matches!(self.busy, Busy::Testing { .. } | Busy::UpdatingZapret(_) | Busy::UpdatingApp(_));

        egui::Frame::new().fill(pal.surface_container_low).corner_radius(CornerRadius::same(16)).inner_margin(egui::Margin::same(4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            for p in &self.profiles {
                let is_sel = selected.as_deref() == Some(&p.id);
                let w = ui.available_width();
                let (rect, resp) = ui.allocate_exact_size(vec2(w, 56.0), if locked { Sense::hover() } else { Sense::click() });
                let radius = CornerRadius::same(12);
                if is_sel {
                    ui.painter().rect_filled(rect, radius, pal.secondary_container);
                }
                widgets::state_layer(ui, &resp, rect, radius, pal.on_surface);

                // радио-кнопка MD3
                let rc = pos2(rect.left() + 26.0, rect.center().y);
                let col = if is_sel { pal.primary } else { pal.on_surface_variant };
                ui.painter().circle_stroke(rc, 9.0, Stroke::new(2.0, col));
                if is_sel {
                    ui.painter().circle_filled(rc, 5.0, col);
                }

                let fg = if is_sel { pal.on_secondary_container } else { pal.on_surface };
                widgets::text_at(ui, pos2(rect.left() + 52.0, rect.center().y - 9.0), Align2::LEFT_CENTER, &p.title, theme::semibold(15.0), fg);
                let mut subtitle = format!("{}.bat", p.id);
                if best.as_deref() == Some(&p.id) {
                    subtitle = format!("Рекомендуется · {subtitle}");
                }
                widgets::text_at(ui, pos2(rect.left() + 52.0, rect.center().y + 10.0), Align2::LEFT_CENTER, &subtitle, theme::regular(12.0), pal.on_surface_variant);

                // результат проверки
                let right = pos2(rect.right() - 14.0, rect.center().y);
                if testing_now.as_deref() == Some(&p.id) {
                    let time = ui.input(|i| i.time) as f32;
                    let pts: Vec<_> = (0..=20)
                        .map(|i| {
                            let a = time * 5.0 + 4.0 * i as f32 / 20.0;
                            pos2(right.x - 9.0 + 8.0 * a.cos(), right.y + 8.0 * a.sin())
                        })
                        .collect();
                    ui.painter().add(egui::Shape::line(pts, Stroke::new(2.5, pal.primary)));
                } else if let Some(s) = self.cfg.scores.get(&p.id) {
                    let txt = if s.ok > 0 { format!("{}/{} · {} мс", s.ok, s.total, s.avg_ms) } else { format!("0/{}", s.total) };
                    let (bg, fg) = if s.total > 0 && s.ok == s.total {
                        (pal.success_container, pal.on_success_container)
                    } else if s.ok * 2 >= s.total && s.ok > 0 {
                        (pal.tertiary_container, pal.on_tertiary_container)
                    } else {
                        (pal.error_container, pal.on_error_container)
                    };
                    let g = ui.painter().layout_no_wrap(txt, theme::semibold(12.0), fg);
                    let r = Rect::from_min_size(pos2(right.x - g.size().x - 16.0, right.y - 12.0), vec2(g.size().x + 16.0, 24.0));
                    ui.painter().rect_filled(r, CornerRadius::same(8), bg);
                    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
                }

                if resp.clicked() {
                    clicked = Some(p.id.clone());
                }
                if !locked {
                    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                }
            }
        });
        if let Some(id) = clicked {
            self.select_profile(id);
        }
    }

    fn settings_section(ui: &mut Ui, pal: &Palette, title: &str) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(title).font(theme::semibold(14.0)).color(pal.primary));
        });
        ui.add_space(2.0);
    }

    fn setting_row(ui: &mut Ui, pal: &Palette, title: &str, subtitle: &str, add: impl FnOnce(&mut Ui)) {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, 56.0), Layout::right_to_left(Align::Center), |ui| {
            ui.set_min_height(56.0);
            add(ui);
            ui.add_space(8.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.add_space(8.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(egui::RichText::new(title).font(theme::regular(16.0)).color(pal.on_surface)).wrap());
                if !subtitle.is_empty() {
                    ui.add(egui::Label::new(egui::RichText::new(subtitle).font(theme::regular(13.0)).color(pal.on_surface_variant)).wrap());
                }
                ui.add_space(6.0);
            });
        });
    }

    fn settings(&mut self, ui: &mut Ui, pal: &Palette) {
        Self::settings_section(ui, pal, "Подключение");
        ui.label(egui::RichText::new("Игровой фильтр").font(theme::regular(16.0)).color(pal.on_surface));
        ui.label(
            egui::RichText::new("Обход для игр и приложений на портах 1024-65535. Включайте, только если без него не работает.")
                .font(theme::regular(13.0))
                .color(pal.on_surface_variant),
        );
        ui.add_space(4.0);
        let labels: Vec<&str> = GameFilter::ALL.iter().map(|g| g.label()).collect();
        let cur = GameFilter::ALL.iter().position(|g| *g == self.cfg.game_filter).unwrap_or(0);
        if let Some(i) = widgets::segmented(ui, pal, &labels, cur) {
            self.cfg.game_filter = GameFilter::ALL[i];
            self.cfg.save();
            if self.connected && !self.is_busy() {
                self.connect();
            }
        }
        ui.add_space(4.0);

        let mut v = self.cfg.autoconnect;
        Self::setting_row(ui, pal, "Подключаться при запуске", "Сразу включать выбранный профиль", |ui| {
            widgets::switch(ui, pal, &mut v);
        });
        if v != self.cfg.autoconnect {
            self.cfg.autoconnect = v;
            self.cfg.save();
        }
        let mut v = self.cfg.auto_switch;
        Self::setting_row(ui, pal, "Автопереключение профиля", "Раз в 3 минуты проверять сайты и при сбое переключаться на рабочий профиль", |ui| {
            widgets::switch(ui, pal, &mut v);
        });
        if v != self.cfg.auto_switch {
            self.cfg.auto_switch = v;
            self.cfg.save();
        }
        let mut v = self.cfg.disconnect_on_exit;
        Self::setting_row(ui, pal, "Отключать при закрытии", "Иначе обход продолжит работать в фоне", |ui| {
            widgets::switch(ui, pal, &mut v);
        });
        if v != self.cfg.disconnect_on_exit {
            self.cfg.disconnect_on_exit = v;
            self.cfg.save();
        }

        Self::settings_section(ui, pal, "Система");
        let mut v = self.cfg.autostart;
        Self::setting_row(ui, pal, "Запускать вместе с Windows", "Через Планировщик заданий, без запроса UAC", |ui| {
            widgets::switch(ui, pal, &mut v);
        });
        if v != self.cfg.autostart {
            match sys::set_autostart(v) {
                Ok(()) => {
                    self.cfg.autostart = v;
                    self.cfg.save();
                }
                Err(e) => self.notify(format!("Не удалось изменить автозапуск: {e}")),
            }
        }
        ui.label(egui::RichText::new("Тема").font(theme::regular(16.0)).color(pal.on_surface));
        ui.add_space(4.0);
        let labels: Vec<&str> = ThemeMode::ALL.iter().map(|t| t.label()).collect();
        let cur = ThemeMode::ALL.iter().position(|t| *t == self.cfg.theme).unwrap_or(0);
        if let Some(i) = widgets::segmented(ui, pal, &labels, cur) {
            self.cfg.theme = ThemeMode::ALL[i];
            self.cfg.save();
            apply_theme_pref(ui.ctx(), self.cfg.theme);
        }

        Self::settings_section(ui, pal, "Обновления");
        let mut v = self.cfg.check_updates;
        Self::setting_row(ui, pal, "Проверять при запуске", "Обновления zapret и приложения", |ui| {
            widgets::switch(ui, pal, &mut v);
        });
        if v != self.cfg.check_updates {
            self.cfg.check_updates = v;
            self.cfg.save();
        }

        let zl = self.zapret_local.clone().unwrap_or_else(|| "не установлен".into());
        let zr = match (&self.zapret_remote, &self.zapret_check_error) {
            (Some(r), _) => format!("последняя: {r}"),
            (None, Some(_)) => "не удалось проверить".into(),
            _ => "проверка…".into(),
        };
        let enabled = !self.is_busy();
        let mut upd_z = false;
        let mut chk = false;
        Self::setting_row(ui, pal, &format!("zapret {zl}"), &format!("flowseal/zapret-discord-youtube · {zr}"), |ui| {
            let need = self.zapret_remote.as_deref().map(|r| self.zapret_local.as_deref().map(|l| updater::is_newer(r, l)).unwrap_or(true)).unwrap_or(false)
                || !paths::zapret_installed();
            if need {
                upd_z = widgets::button(ui, pal, ButtonKind::Filled, None, "Обновить", enabled).clicked();
            } else {
                upd_z = widgets::button(ui, pal, ButtonKind::Text, None, "Переустановить", enabled).clicked();
            }
        });
        let app_sub = match &self.app_update {
            Some(u) => format!("доступна версия {}", u.version),
            None if self.app_checked => "установлена последняя версия".into(),
            None => "проверка…".into(),
        };
        let mut upd_a = false;
        Self::setting_row(ui, pal, &format!("Zapret PF {}", updater::APP_VERSION), &app_sub, |ui| {
            if let Some(u) = &self.app_update {
                upd_a = widgets::button(ui, pal, ButtonKind::Filled, None, "Установить", enabled).clicked();
                if widgets::button(ui, pal, ButtonKind::Text, None, "Что нового", true).clicked() {
                    sys::open_url(&u.page);
                }
            } else {
                chk = widgets::button(ui, pal, ButtonKind::Text, None, "Проверить", enabled).clicked();
            }
        });
        let mut upd_ip = false;
        Self::setting_row(ui, pal, "Список IPSet", "Обновить lists\\ipset-all.txt", |ui| {
            upd_ip = widgets::button(ui, pal, ButtonKind::Text, None, "Обновить", enabled && paths::zapret_installed()).clicked();
        });
        if upd_z {
            self.update_zapret();
        }
        if upd_a {
            self.update_app();
        }
        if chk {
            self.check_updates(true);
        }
        if upd_ip {
            self.bg(|tx, ctx| {
                let m = match updater::update_ipset() {
                    Ok(()) => "Список IPSet обновлён".to_string(),
                    Err(e) => format!("Ошибка обновления IPSet: {e}"),
                };
                Self::send(&tx, &ctx, Msg::Info(m));
            });
        }

        Self::settings_section(ui, pal, "Инструменты");
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui, pal, ButtonKind::Outlined, Some(Icon::Folder), "Папка zapret", true).clicked() {
                let _ = std::fs::create_dir_all(paths::zapret_dir());
                sys::open_path(&paths::zapret_dir());
            }
            if widgets::button(ui, pal, ButtonKind::Outlined, Some(Icon::Folder), "Списки", paths::zapret_installed()).clicked() {
                zapret::ensure_user_lists();
                sys::open_path(&paths::lists_dir());
            }
            if widgets::button(ui, pal, ButtonKind::Outlined, None, "Журнал winws", paths::log_file().exists()).clicked() {
                sys::open_path(&paths::log_file());
            }
            if widgets::button(ui, pal, ButtonKind::Outlined, None, "Обновить список профилей", !self.is_busy()).clicked() {
                self.profiles = profiles::load_profiles();
                self.notify(format!("Найдено профилей: {}", self.profiles.len()));
            }
        });
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "Свои домены добавляйте в lists\\list-general-user.txt, исключения — в list-exclude-user.txt. \
                 Файлы сохраняются при обновлении zapret.",
            )
            .font(theme::regular(13.0))
            .color(pal.on_surface_variant),
        );

        Self::settings_section(ui, pal, "О программе");
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui, pal, ButtonKind::Text, None, "Zapret PF на GitHub", true).clicked() {
                sys::open_url(&format!("https://github.com/{}", updater::APP_REPO));
            }
            if widgets::button(ui, pal, ButtonKind::Text, None, "zapret-discord-youtube", true).clicked() {
                sys::open_url(&format!("https://github.com/{}", updater::ZAPRET_REPO));
            }
        });
        ui.add_space(24.0);
    }

    fn snackbar(&mut self, ui: &mut Ui, pal: &Palette) {
        let Some((text, t)) = self.snackbar.clone() else { return };
        let age = t.elapsed().as_secs_f32();
        let life = 5.0;
        if age > life {
            self.snackbar = None;
            return;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        let screen = ui.ctx().content_rect();
        let w = (screen.width() - 32.0).min(560.0);
        let font = theme::regular(14.0);
        let galley = ui.painter().layout(text, font, pal.inverse_on_surface, w - 32.0);
        let h = galley.size().y + 28.0;
        let appear = (age / 0.2).min(1.0).min((life - age) / 0.3);
        let rect = Rect::from_min_size(pos2(screen.center().x - w / 2.0, screen.bottom() - h - 16.0 + (1.0 - appear) * 12.0), vec2(w, h));
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("snackbar")));
        painter.add(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha((60.0 * appear) as u8) }.as_shape(rect, CornerRadius::same(8)));
        painter.rect(rect, CornerRadius::same(8), pal.inverse_surface.gamma_multiply(appear), Stroke::NONE, StrokeKind::Inside);
        painter.galley(pos2(rect.left() + 16.0, rect.top() + 14.0), galley, pal.inverse_on_surface);
        let resp = ui.interact(rect, egui::Id::unique("snackbar-click"), Sense::click());
        if resp.clicked() {
            self.snackbar = None;
        }
    }
}

fn apply_theme_pref(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_theme(match mode {
        ThemeMode::System => egui::ThemePreference::System,
        ThemeMode::Light => egui::ThemePreference::Light,
        ThemeMode::Dark => egui::ThemePreference::Dark,
    });
}

impl eframe::App for ZapretApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);
        self.maybe_health_check();
        if self.cfg.auto_switch && self.connected {
            // logic() вызывается и при свёрнутом окне — будим его для проверок
            ctx.request_repaint_after(Duration::from_secs(15));
        }
        if self.minimize_once {
            self.minimize_once = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let theme: Theme = ui.ctx().theme();
        let pal = *theme::palette(theme);

        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal.surface)).show(ui, |ui| {
            self.top_bar(ui, &pal);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 16, top: 0, bottom: 16 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.banners(ui, &pal);
                    match self.screen {
                        Screen::Main => {
                            self.hero(ui, &pal);
                            self.profile_list(ui, &pal);
                            ui.add_space(12.0);
                            ui.vertical_centered(|ui| {
                                let z = self.zapret_local.clone().unwrap_or_else(|| "—".into());
                                ui.label(
                                    egui::RichText::new(format!("zapret {z} · Zapret PF {}", updater::APP_VERSION))
                                        .font(theme::regular(12.0))
                                        .color(pal.on_surface_variant.gamma_multiply(0.8)),
                                );
                            });
                        }
                        Screen::Settings => self.settings(ui, &pal),
                    }
                });
            });
        });
        self.snackbar(ui, &pal);
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.cfg.save();
        if self.disconnect_on_exit_needed() {
            zapret::stop();
        }
    }
}

impl ZapretApp {
    fn disconnect_on_exit_needed(&self) -> bool {
        self.cfg.disconnect_on_exit || matches!(self.busy, Busy::Testing { .. })
    }
}
