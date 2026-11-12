//! Небольшой набор виджетов в стиле Material Design 3 поверх egui.

use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use std::f32::consts::{PI, TAU};

use crate::theme::{self, Palette};

// ------------------------------------------------------------------ icons

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Power,
    Gear,
    Back,
    Refresh,
    Sparkle,
    Check,
    Download,
    Folder,
    Info,
    Warning,
}

fn arc_points(c: Pos2, r: f32, from: f32, to: f32, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

pub fn paint_icon(ui: &Ui, icon: Icon, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let w = (s * 0.1).max(1.6);
    let stroke = Stroke::new(w, color);
    match icon {
        Icon::Power => {
            let r = s * 0.36;
            let gap = 0.30 * PI;
            p.add(Shape::line(arc_points(c, r, -PI / 2.0 + gap, 1.5 * PI - gap, 40), stroke));
            p.line_segment([pos2(c.x, c.y - s * 0.46), pos2(c.x, c.y - s * 0.06)], stroke);
        }
        Icon::Gear => {
            let teeth = 8;
            let (ro, ri) = (s * 0.44, s * 0.32);
            let mut pts = Vec::new();
            for i in 0..teeth * 4 {
                let a = i as f32 / (teeth * 4) as f32 * TAU;
                let r = if (i / 2) % 2 == 0 { ro } else { ri };
                pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
            }
            p.add(Shape::closed_line(pts, Stroke::new(w * 0.9, color)));
            p.circle_stroke(c, s * 0.13, Stroke::new(w * 0.9, color));
        }
        Icon::Back => {
            let l = s * 0.32;
            p.line_segment([pos2(c.x - l, c.y), pos2(c.x + l, c.y)], stroke);
            p.add(Shape::line(
                vec![pos2(c.x - l * 0.1, c.y - l * 0.9), pos2(c.x - l, c.y), pos2(c.x - l * 0.1, c.y + l * 0.9)],
                stroke,
            ));
        }
        Icon::Refresh => {
            let r = s * 0.3;
            let pts = arc_points(c, r, -PI * 0.35, PI * 1.45, 40);
            let end = pts[0];
            p.add(Shape::line(pts, stroke));
            let a = s * 0.16;
            p.add(Shape::convex_polygon(
                vec![pos2(end.x + a * 0.2, end.y - a * 1.1), pos2(end.x + a * 1.1, end.y + a * 0.3), pos2(end.x - a * 0.8, end.y + a * 0.25)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Sparkle => {
            let star = |cx: f32, cy: f32, r: f32| {
                let k = r * 0.28;
                vec![
                    pos2(cx, cy - r),
                    pos2(cx + k, cy - k),
                    pos2(cx + r, cy),
                    pos2(cx + k, cy + k),
                    pos2(cx, cy + r),
                    pos2(cx - k, cy + k),
                    pos2(cx - r, cy),
                    pos2(cx - k, cy - k),
                ]
            };
            let fan = |pts: Vec<Pos2>, cx: f32, cy: f32| {
                for i in 0..pts.len() {
                    let a = pts[i];
                    let b = pts[(i + 1) % pts.len()];
                    p.add(Shape::convex_polygon(vec![pos2(cx, cy), a, b], color, Stroke::NONE));
                }
            };
            fan(star(c.x - s * 0.08, c.y + s * 0.06, s * 0.34), c.x - s * 0.08, c.y + s * 0.06);
            fan(star(c.x + s * 0.26, c.y - s * 0.26, s * 0.16), c.x + s * 0.26, c.y - s * 0.26);
        }
        Icon::Check => {
            p.add(Shape::line(
                vec![pos2(c.x - s * 0.3, c.y), pos2(c.x - s * 0.08, c.y + s * 0.22), pos2(c.x + s * 0.32, c.y - s * 0.22)],
                stroke,
            ));
        }
        Icon::Download => {
            p.line_segment([pos2(c.x, c.y - s * 0.36), pos2(c.x, c.y + s * 0.12)], stroke);
            p.add(Shape::line(
                vec![pos2(c.x - s * 0.2, c.y - s * 0.08), pos2(c.x, c.y + s * 0.12), pos2(c.x + s * 0.2, c.y - s * 0.08)],
                stroke,
            ));
            p.line_segment([pos2(c.x - s * 0.32, c.y + s * 0.34), pos2(c.x + s * 0.32, c.y + s * 0.34)], stroke);
        }
        Icon::Folder => {
            let r = Rect::from_center_size(c + vec2(0.0, s * 0.04), vec2(s * 0.72, s * 0.52));
            p.rect_stroke(r, 2.0, stroke, StrokeKind::Middle);
            p.line_segment([r.left_top() + vec2(0.0, 0.0), r.left_top() + vec2(s * 0.24, 0.0)], Stroke::new(w * 2.0, color));
        }
        Icon::Info | Icon::Warning => {
            p.circle_stroke(c, s * 0.38, stroke);
            if icon == Icon::Info {
                p.line_segment([pos2(c.x, c.y - s * 0.02), pos2(c.x, c.y + s * 0.2)], stroke);
                p.circle_filled(pos2(c.x, c.y - s * 0.17), w * 0.7, color);
            } else {
                p.line_segment([pos2(c.x, c.y - s * 0.2), pos2(c.x, c.y + s * 0.04)], stroke);
                p.circle_filled(pos2(c.x, c.y + s * 0.18), w * 0.7, color);
            }
        }
    }
}

// ------------------------------------------------------------------ buttons

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonKind {
    Filled,
    Tonal,
    Outlined,
    Text,
}

/// MD3 кнопка (высота 40dp, полностью скруглённая).
pub fn button(ui: &mut Ui, pal: &Palette, kind: ButtonKind, icon: Option<Icon>, label: &str, enabled: bool) -> Response {
    let font = theme::semibold(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, Color32::WHITE);
    let icon_w = if icon.is_some() { 26.0 } else { 0.0 };
    let pad = if kind == ButtonKind::Text { 12.0 } else { 22.0 };
    let size = vec2(galley.size().x + pad * 2.0 + icon_w - if icon.is_some() { 6.0 } else { 0.0 }, 40.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let (bg, fg, stroke) = match kind {
            ButtonKind::Filled => (pal.primary, pal.on_primary, Stroke::NONE),
            ButtonKind::Tonal => (pal.secondary_container, pal.on_secondary_container, Stroke::NONE),
            ButtonKind::Outlined => (Color32::TRANSPARENT, pal.primary, Stroke::new(1.0, pal.outline)),
            ButtonKind::Text => (Color32::TRANSPARENT, pal.primary, Stroke::NONE),
        };
        let (bg, fg) = if enabled {
            (bg, fg)
        } else {
            (
                if bg == Color32::TRANSPARENT { bg } else { pal.on_surface.gamma_multiply(0.12) },
                pal.on_surface.gamma_multiply(0.38),
            )
        };
        let radius = CornerRadius::same(20);
        ui.painter().rect(rect, radius, bg, stroke, StrokeKind::Inside);
        state_layer(ui, &resp, rect, radius, fg);
        let mut x = rect.left() + pad;
        if let Some(ic) = icon {
            paint_icon(ui, ic, Rect::from_center_size(pos2(x + 9.0 - 6.0, rect.center().y), vec2(18.0, 18.0)), fg);
            x += icon_w - 6.0;
        }
        ui.painter().galley_with_override_text_color(pos2(x, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Круглая иконка-кнопка 40dp.
pub fn icon_button(ui: &mut Ui, pal: &Palette, icon: Icon, tooltip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
    state_layer(ui, &resp, rect, CornerRadius::same(20), pal.on_surface_variant);
    paint_icon(ui, icon, Rect::from_center_size(rect.center(), vec2(22.0, 22.0)), pal.on_surface_variant);
    resp.on_hover_text(tooltip).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Слой состояния MD3: 8% при наведении, 12% при нажатии.
pub fn state_layer(ui: &Ui, resp: &Response, rect: Rect, radius: CornerRadius, color: Color32) {
    let a = if resp.is_pointer_button_down_on() {
        0.12
    } else if resp.hovered() && resp.sense.senses_click() {
        0.08
    } else {
        0.0
    };
    if a > 0.0 {
        ui.painter().rect_filled(rect, radius, color.gamma_multiply(a));
    }
}

/// MD3 Switch (52×32dp).
pub fn switch(ui: &mut Ui, pal: &Palette, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.15);
    let p = ui.painter();
    let track = lerp_color(pal.surface_container_highest, pal.primary, t);
    let border = lerp_color(pal.outline, pal.primary, t);
    p.rect(rect, CornerRadius::same(16), track, Stroke::new(2.0, border), StrokeKind::Inside);
    let r = egui::lerp(8.0..=12.0, t) + if resp.is_pointer_button_down_on() { 2.0 } else { 0.0 };
    let x = egui::lerp((rect.left() + 16.0)..=(rect.right() - 16.0), t);
    let thumb = lerp_color(pal.outline, pal.on_primary, t);
    if resp.hovered() {
        p.circle_filled(pos2(x, rect.center().y), 20.0, pal.on_surface.gamma_multiply(0.08));
    }
    p.circle_filled(pos2(x, rect.center().y), r, thumb);
    if t > 0.5 {
        paint_icon(ui, Icon::Check, Rect::from_center_size(pos2(x, rect.center().y), vec2(16.0, 16.0)), pal.primary.gamma_multiply((t - 0.5) * 2.0));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// MD3 Segmented button. Возвращает индекс, если выбор изменился.
pub fn segmented(ui: &mut Ui, pal: &Palette, items: &[&str], selected: usize) -> Option<usize> {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 40.0), Sense::hover());
    let n = items.len().max(1);
    let seg_w = rect.width() / n as f32;
    let mut changed = None;
    for (i, label) in items.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + seg_w * i as f32, rect.top()), vec2(seg_w, 40.0));
        let resp = ui.interact(r, ui.scope_id().with(("seg", i, label)), Sense::click());
        let radius = CornerRadius {
            nw: if i == 0 { 20 } else { 0 },
            sw: if i == 0 { 20 } else { 0 },
            ne: if i == n - 1 { 20 } else { 0 },
            se: if i == n - 1 { 20 } else { 0 },
        };
        let sel = i == selected;
        if sel {
            ui.painter().rect_filled(r, radius, pal.secondary_container);
        }
        state_layer(ui, &resp, r, radius, pal.on_surface);
        let fg = if sel { pal.on_secondary_container } else { pal.on_surface };
        let font = theme::semibold(13.0);
        let g = ui.painter().layout_no_wrap(label.to_string(), font, fg);
        let icon_w = if sel { 20.0 } else { 0.0 };
        let total = g.size().x + icon_w;
        let x0 = r.center().x - total / 2.0;
        if sel {
            paint_icon(ui, Icon::Check, Rect::from_center_size(pos2(x0 + 7.0, r.center().y), vec2(16.0, 16.0)), fg);
        }
        ui.painter().galley(pos2(x0 + icon_w, r.center().y - g.size().y / 2.0), g, fg);
        if resp.clicked() && !sel {
            changed = Some(i);
        }
        if i > 0 {
            ui.painter().line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, pal.outline));
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    ui.painter().rect_stroke(rect, CornerRadius::same(20), Stroke::new(1.0, pal.outline), StrokeKind::Inside);
    changed
}

/// Линейный индикатор прогресса MD3 (определённый или неопределённый).
pub fn linear_progress(ui: &mut Ui, pal: &Palette, value: Option<f32>) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 4.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(2), pal.secondary_container);
    match value {
        Some(v) => {
            let r = Rect::from_min_size(rect.min, vec2(rect.width() * v.clamp(0.0, 1.0), rect.height()));
            p.rect_filled(r, CornerRadius::same(2), pal.primary);
        }
        None => {
            let t = (ui.input(|i| i.time) as f32 * 0.7).fract();
            let x0 = rect.left() + rect.width() * (t * 1.4 - 0.4);
            let r = Rect::from_x_y_ranges(x0.max(rect.left())..=(x0 + rect.width() * 0.4).min(rect.right()), rect.y_range());
            p.rect_filled(r, CornerRadius::same(2), pal.primary);
            ui.ctx().request_repaint();
        }
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Текст, выровненный внутри прямоугольника.
pub fn text_at(ui: &Ui, pos: Pos2, align: Align2, text: &str, font: egui::FontId, color: Color32) -> Rect {
    ui.painter().text(pos, align, text, font, color)
}
