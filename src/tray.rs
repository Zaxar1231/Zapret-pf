//! Иконка в области уведомлений Windows: показывает статус, позволяет
//! подключаться/отключаться и выходить. Окно при закрытии прячется сюда,
//! а слежение за профилем продолжает работать.

use std::sync::mpsc::Sender;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::icon;

pub enum TrayCmd {
    Show,
    Toggle,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    status: MenuItem,
    toggle: MenuItem,
    last: Option<(bool, String)>,
}

fn make_icon(rgb: (u8, u8, u8)) -> Option<Icon> {
    Icon::from_rgba(icon::power_icon_rgba(32, rgb), 32, 32).ok()
}

impl Tray {
    pub fn new(tx: Sender<TrayCmd>, ctx: egui::Context) -> Option<Self> {
        let status = MenuItem::new("Zapret PF", false, None);
        let open = MenuItem::new("Открыть", true, None);
        let toggle = MenuItem::new("Подключить", true, None);
        let quit = MenuItem::new("Выход", true, None);
        let menu = Menu::new();
        menu.append_items(&[&status, &PredefinedMenuItem::separator(), &open, &toggle, &PredefinedMenuItem::separator(), &quit])
            .ok()?;

        let (open_id, toggle_id, quit_id) = (open.id().clone(), toggle.id().clone(), quit.id().clone());
        {
            let tx = tx.clone();
            let ctx = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let cmd = if e.id == open_id {
                    TrayCmd::Show
                } else if e.id == toggle_id {
                    TrayCmd::Toggle
                } else if e.id == quit_id {
                    TrayCmd::Quit
                } else {
                    return;
                };
                let _ = tx.send(cmd);
                ctx.request_repaint();
            }));
        }
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            let show = matches!(
                e,
                TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
                    | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. }
            );
            if show {
                let _ = tx.send(TrayCmd::Show);
                ctx.request_repaint();
            }
        }));

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Zapret PF")
            .with_icon(make_icon(icon::COLOR_OFF)?)
            .build()
            .ok()?;
        Some(Self { icon, status, toggle, last: None })
    }

    /// Обновляет иконку, подсказку и пункты меню (только при изменении).
    pub fn update(&mut self, connected: bool, status: &str, toggle_enabled: bool) {
        let key = (connected, format!("{status}|{toggle_enabled}"));
        if self.last.as_ref() == Some(&key) {
            return;
        }
        let _ = self.icon.set_icon(make_icon(if connected { icon::COLOR_ON } else { icon::COLOR_OFF }));
        let _ = self.icon.set_tooltip(Some(format!("Zapret PF — {status}")));
        self.status.set_text(status);
        self.toggle.set_text(if connected { "Отключить" } else { "Подключить" });
        self.toggle.set_enabled(toggle_enabled);
        self.last = Some(key);
    }
}
