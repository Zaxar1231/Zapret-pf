//! Material Design 3: цветовые роли (тон-палитра от seed #3F6BD8 — «VPN-синий»),
//! скругления и типографика.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle, Theme};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub struct Palette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub surface: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub inverse_surface: Color32,
    pub inverse_on_surface: Color32,
    pub success_container: Color32,
    pub on_success_container: Color32,
}

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const LIGHT: Palette = Palette {
    primary: c(0x3F5FA9),
    on_primary: c(0xFFFFFF),
    primary_container: c(0xDAE2FF),
    on_primary_container: c(0x001946),
    secondary_container: c(0xDCE2F9),
    on_secondary_container: c(0x141B2C),
    tertiary_container: c(0xFBD7FC),
    on_tertiary_container: c(0x29132D),
    error_container: c(0xFFDAD6),
    on_error_container: c(0x410002),
    surface: c(0xFAF8FF),
    surface_container_low: c(0xF4F3FA),
    surface_container: c(0xEEEDF4),
    surface_container_high: c(0xE8E7EF),
    surface_container_highest: c(0xE2E2E9),
    on_surface: c(0x1A1B20),
    on_surface_variant: c(0x44464F),
    outline: c(0x757780),
    outline_variant: c(0xC5C6D0),
    inverse_surface: c(0x2F3036),
    inverse_on_surface: c(0xF1F0F7),
    success_container: c(0xB1F2A8),
    on_success_container: c(0x002204),
};

pub const DARK: Palette = Palette {
    primary: c(0xB1C5FF),
    on_primary: c(0x002C70),
    primary_container: c(0x264690),
    on_primary_container: c(0xDAE2FF),
    secondary_container: c(0x3F4759),
    on_secondary_container: c(0xDCE2F9),
    tertiary_container: c(0x583E5B),
    on_tertiary_container: c(0xFBD7FC),
    error_container: c(0x93000A),
    on_error_container: c(0xFFDAD6),
    surface: c(0x121318),
    surface_container_low: c(0x1A1B21),
    surface_container: c(0x1E1F25),
    surface_container_high: c(0x282A2F),
    surface_container_highest: c(0x33353A),
    on_surface: c(0xE2E2E9),
    on_surface_variant: c(0xC5C6D0),
    outline: c(0x8F9099),
    outline_variant: c(0x44464F),
    inverse_surface: c(0xE2E2E9),
    inverse_on_surface: c(0x2F3036),
    success_container: c(0x14521B),
    on_success_container: c(0xB1F2A8),
};

pub fn palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Light => &LIGHT,
        Theme::Dark => &DARK,
    }
}

pub const SEMIBOLD: &str = "semibold";

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Подключает системные шрифты Windows (Segoe UI — отличная кириллица),
/// с запасным вариантом на встроенные шрифты egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let fdir = std::path::Path::new(&win).join("Fonts");
    let load = |names: &[&str]| -> Option<Vec<u8>> {
        names.iter().find_map(|n| std::fs::read(fdir.join(n)).ok())
    };

    let mut regular_stack: Vec<String> = Vec::new();
    let mut semibold_stack: Vec<String> = Vec::new();

    if let Some(b) = load(&["segoeui.ttf"]) {
        fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(b)));
        regular_stack.push("segoe".into());
    }
    if let Some(b) = load(&["seguisb.ttf", "segoeuib.ttf"]) {
        fonts.font_data.insert("segoe-sb".into(), Arc::new(FontData::from_owned(b)));
        semibold_stack.push("segoe-sb".into());
    }
    if let Some(b) = load(&["seguisym.ttf"]) {
        fonts.font_data.insert("segoe-sym".into(), Arc::new(FontData::from_owned(b)));
    }

    let defaults = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = regular_stack.clone();
    prop.extend(defaults.iter().cloned());
    if fonts.font_data.contains_key("segoe-sym") {
        prop.push("segoe-sym".into());
    }
    let mut sb = semibold_stack;
    sb.extend(prop.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name(SEMIBOLD.into()), sb);
    ctx.set_fonts(fonts);
}

fn visuals_for(theme: Theme) -> egui::Visuals {
    let p = palette(theme);
    let mut v = match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    v.panel_fill = p.surface;
    v.window_fill = p.surface_container_high;
    v.extreme_bg_color = p.surface_container_highest;
    v.faint_bg_color = p.surface_container_low;
    v.override_text_color = Some(p.on_surface);
    v.hyperlink_color = p.primary;
    v.selection.bg_fill = p.primary.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, p.primary);
    v.window_corner_radius = 28.into();
    v.menu_corner_radius = 12.into();
    v.window_stroke = egui::Stroke::NONE;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = 20.into();
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.outline_variant);
    v.widgets.inactive.weak_bg_fill = p.secondary_container;
    v.widgets.inactive.bg_fill = p.surface_container_highest;
    v.widgets.hovered.weak_bg_fill = p.secondary_container;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.secondary_container;
    v
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals_of(Theme::Light, visuals_for(Theme::Light));
    ctx.set_visuals_of(Theme::Dark, visuals_for(Theme::Dark));
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, regular(12.0)),
            (TextStyle::Body, regular(14.5)),
            (TextStyle::Button, semibold(14.0)),
            (TextStyle::Heading, semibold(22.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(16.0, 8.0);
        s.spacing.scroll.bar_width = 6.0;
        s.spacing.scroll.floating = true;
        s.interaction.selectable_labels = false;
    });
}
