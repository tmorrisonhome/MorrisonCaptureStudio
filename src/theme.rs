//! Colours, fonts and the custom-drawn widgets that give the app its look.
#![allow(dead_code)]
use crate::icons::{self, Icon};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, UiBuilder,
};

pub const BG: Color32 = Color32::from_rgb(10, 15, 23);
pub const HEADER: Color32 = Color32::from_rgb(13, 20, 31);
pub const PANEL: Color32 = Color32::from_rgb(19, 27, 38);
pub const PANEL_BORDER: Color32 = Color32::from_rgb(36, 48, 64);
pub const FIELD: Color32 = Color32::from_rgb(27, 36, 49);
pub const FIELD_HOVER: Color32 = Color32::from_rgb(35, 47, 64);
pub const FIELD_BORDER: Color32 = Color32::from_rgb(50, 63, 82);
pub const TEXT: Color32 = Color32::from_rgb(232, 238, 246);
pub const DIM: Color32 = Color32::from_rgb(150, 164, 182);
pub const BLUE: Color32 = Color32::from_rgb(43, 121, 232);
pub const BLUE_HOVER: Color32 = Color32::from_rgb(62, 138, 245);
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(79, 179, 255);
pub const SELECT: Color32 = Color32::from_rgb(28, 50, 84);
pub const RED: Color32 = Color32::from_rgb(229, 52, 58);
pub const RED_HOVER: Color32 = Color32::from_rgb(242, 72, 78);
pub const GREEN: Color32 = Color32::from_rgb(46, 204, 106);
pub const ORANGE: Color32 = Color32::from_rgb(245, 166, 35);
pub const WAVE_L: Color32 = Color32::from_rgb(45, 140, 255);
pub const WAVE_R: Color32 = Color32::from_rgb(40, 214, 110);

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn semi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}
pub fn reg(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Uses Windows' Segoe UI (as in the design) when available, with egui's fonts as backup.
pub fn setup(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let load = |file: &str| std::fs::read(format!("{windir}\\Fonts\\{file}")).ok();
    let mut add = |name: &str, files: &[&str]| -> bool {
        for f in files {
            if let Some(b) = load(f) {
                fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(b)));
                return true;
            }
        }
        false
    };
    let has_reg = add("segoe", &["segoeui.ttf"]);
    let has_semi = add("segoe-semi", &["seguisb.ttf", "segoeuib.ttf"]);
    let has_bold = add("segoe-bold", &["segoeuib.ttf", "seguisb.ttf"]);
    let has_sym = add("segoe-sym", &["seguisym.ttf"]);
    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop = Vec::new();
    if has_reg {
        prop.push("segoe".to_string());
    }
    prop.extend(base.iter().cloned());
    if has_sym {
        prop.push("segoe-sym".into());
    }
    let mut semiv = Vec::new();
    if has_semi {
        semiv.push("segoe-semi".to_string());
    }
    semiv.extend(prop.iter().cloned());
    let mut boldv = Vec::new();
    if has_bold {
        boldv.push("segoe-bold".to_string());
    }
    boldv.extend(prop.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name("semi".into()), semiv);
    fonts.families.insert(FontFamily::Name("bold".into()), boldv);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.text_styles.insert(egui::TextStyle::Body, reg(13.5));
    style.text_styles.insert(egui::TextStyle::Button, reg(13.5));
    style.text_styles.insert(egui::TextStyle::Small, reg(11.5));
    style.text_styles.insert(egui::TextStyle::Heading, semi(16.0));
    style.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(12.0));
    style.spacing.item_spacing = vec2(8.0, 7.0);
    style.spacing.button_padding = vec2(10.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.slider_width = 160.0;
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, FIELD_BORDER);
    v.window_corner_radius = CornerRadius::same(8);
    v.extreme_bg_color = Color32::from_rgb(12, 18, 27);
    v.faint_bg_color = FIELD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = BLUE;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = BLUE_LIGHT;
    for (w, fill) in [
        (&mut v.widgets.inactive, FIELD),
        (&mut v.widgets.hovered, FIELD_HOVER),
        (&mut v.widgets.active, SELECT),
        (&mut v.widgets.open, FIELD_HOVER),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, FIELD_BORDER);
        w.fg_stroke = Stroke::new(1.5, TEXT);
        w.corner_radius = CornerRadius::same(5);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, PANEL_BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, BLUE);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
}

/// A child area at an exact position (used to lay the screen out like the design).
pub fn region<R>(ui: &mut Ui, rect: Rect, salt: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(salt).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    f(&mut child)
}

/// Draws a titled panel; returns the area under the title.
pub fn panel(ui: &Ui, rect: Rect, title: &str) -> Rect {
    let pt = ui.painter();
    pt.rect_filled(rect, 6.0, PANEL);
    pt.rect_stroke(rect, 6.0, Stroke::new(1.0, PANEL_BORDER), StrokeKind::Inside);
    if title.is_empty() {
        return rect.shrink(12.0);
    }
    pt.text(pos2(rect.left() + 12.0, rect.top() + 11.0), Align2::LEFT_TOP, title, semi(15.0), TEXT);
    Rect::from_min_max(pos2(rect.left() + 12.0, rect.top() + 38.0), pos2(rect.right() - 12.0, rect.bottom() - 10.0))
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Normal,
    Primary,
    Danger,
    Outline,
    Ghost,
}

/// A button drawn to match the design. `icon` is optional.
pub fn button(ui: &Ui, rect: Rect, id: &str, label: &str, icon: Option<Icon>, kind: Kind, enabled: bool, size: f32) -> Response {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    let hov = enabled && resp.hovered();
    let (fill, stroke, txt) = match kind {
        Kind::Primary => (if hov { BLUE_HOVER } else { BLUE }, Stroke::NONE, Color32::WHITE),
        Kind::Danger => (if hov { RED_HOVER } else { RED }, Stroke::NONE, Color32::WHITE),
        Kind::Normal => (if hov { FIELD_HOVER } else { Color32::from_rgb(38, 48, 63) }, Stroke::new(1.0, FIELD_BORDER), TEXT),
        Kind::Outline => (if hov { FIELD_HOVER } else { Color32::from_rgb(22, 31, 44) }, Stroke::new(1.0, FIELD_BORDER), TEXT),
        Kind::Ghost => (if hov { FIELD_HOVER } else { Color32::TRANSPARENT }, Stroke::NONE, TEXT),
    };
    let (fill, txt) = if enabled { (fill, txt) } else { (Color32::from_rgb(36, 44, 56), Color32::from_rgb(120, 130, 145)) };
    let pt = ui.painter();
    pt.rect_filled(rect, 6.0, fill);
    if stroke != Stroke::NONE {
        pt.rect_stroke(rect, 6.0, stroke, StrokeKind::Inside);
    }
    let font = semi(size);
    let galley = pt.layout_no_wrap(label.to_string(), font, txt);
    let isz = size * 1.25;
    let gap = if icon.is_some() && !label.is_empty() { 9.0 } else { 0.0 };
    let total = galley.size().x + if icon.is_some() { isz + gap } else { 0.0 };
    let mut x = rect.center().x - total / 2.0;
    if let Some(ic) = icon {
        icons::draw(pt, Rect::from_center_size(pos2(x + isz / 2.0, rect.center().y), vec2(isz, isz)), ic, txt);
        x += isz + gap;
    }
    pt.galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, txt);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Square icon-only button with a tooltip.
pub fn icon_button(ui: &Ui, rect: Rect, id: &str, icon: Icon, tip: &str, enabled: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    let pt = ui.painter();
    let fill = if enabled && resp.hovered() { FIELD_HOVER } else { Color32::from_rgb(30, 40, 54) };
    pt.rect_filled(rect, 5.0, fill);
    pt.rect_stroke(rect, 5.0, Stroke::new(1.0, FIELD_BORDER), StrokeKind::Inside);
    let c = if enabled { TEXT } else { DIM };
    icons::draw(pt, rect.shrink(rect.height() * 0.24), icon, c);
    resp.on_hover_text(tip)
}

/// Bare icon (no box) that can be clicked.
pub fn icon_plain(ui: &Ui, rect: Rect, id: &str, icon: Icon, tip: &str) -> Response {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let c = if resp.hovered() { BLUE_LIGHT } else { TEXT };
    icons::draw(ui.painter(), rect, icon, c);
    resp.on_hover_text(tip)
}

/// Big square choice tile with an icon over a label (panel "1. Select What to Record").
pub fn tile(ui: &Ui, rect: Rect, id: &str, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    let pt = ui.painter();
    let hov = enabled && resp.hovered();
    let fill = if selected { SELECT } else if hov { FIELD_HOVER } else { Color32::from_rgb(25, 34, 47) };
    let stroke = if selected { Stroke::new(1.5, BLUE) } else { Stroke::new(1.0, FIELD_BORDER) };
    pt.rect_filled(rect, 6.0, fill);
    pt.rect_stroke(rect, 6.0, stroke, StrokeKind::Inside);
    let c = if enabled { TEXT } else { DIM };
    let isz = (rect.width() * 0.36).min(rect.height() * 0.34);
    icons::draw(pt, Rect::from_center_size(pos2(rect.center().x, rect.top() + rect.height() * 0.32), vec2(isz, isz)), icon, c);
    let top = rect.top() + rect.height() * 0.58;
    // Shrink the text until the longest line fits inside the tile (never break a word).
    let mut size = 12.5;
    while size > 9.0 {
        let widest = label.lines().map(|l| pt.layout_no_wrap(l.to_string(), reg(size), c).size().x).fold(0.0, f32::max);
        if widest <= rect.width() - 8.0 {
            break;
        }
        size -= 0.5;
    }
    let mut lj = egui::text::LayoutJob::simple(label.to_string(), reg(size), c, f32::INFINITY);
    lj.halign = egui::Align::Center;
    let g2 = pt.layout_job(lj);
    pt.galley(pos2(rect.center().x, top), g2, c);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// iOS-style switch. Returns true when flipped.
pub fn toggle(ui: &Ui, rect: Rect, id: &str, on: &mut bool, enabled: bool) -> bool {
    let resp = ui.interact(rect, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    let mut changed = false;
    if resp.clicked() {
        *on = !*on;
        changed = true;
    }
    let t = ui.ctx().animate_bool(ui.id().with(id).with("anim"), *on);
    let pt = ui.painter();
    let off_col = Color32::from_rgb(55, 66, 82);
    let col = lerp_color(off_col, BLUE, t);
    let col = if enabled { col } else { col.gamma_multiply(0.5) };
    let r = rect.height() / 2.0;
    pt.rect_filled(rect, r, col);
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
    pt.circle_filled(pos2(x, rect.center().y), r - 3.0, Color32::WHITE);
    changed
}

/// Toggle with a label on its right, laid out on one line starting at `pos`.
pub fn toggle_row(ui: &Ui, rect: Rect, id: &str, label: &str, on: &mut bool, enabled: bool) -> bool {
    let sw = Rect::from_min_size(pos2(rect.left(), rect.center().y - 9.0), vec2(34.0, 18.0));
    let changed = toggle(ui, sw, id, on, enabled);
    ui.painter().text(pos2(sw.right() + 10.0, rect.center().y), Align2::LEFT_CENTER, label, reg(13.0), if enabled { TEXT } else { DIM });
    // clicking the label also toggles
    let lab = Rect::from_min_max(pos2(sw.right() + 6.0, rect.top()), rect.max);
    if enabled && ui.interact(lab, ui.id().with(id).with("lbl"), Sense::click()).clicked() {
        *on = !*on;
        return true;
    }
    changed
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Horizontal track with a round knob. Returns the new 0..1 value while dragging/clicking,
/// and whether the drag finished this frame.
pub fn slider_bar(ui: &Ui, rect: Rect, id: &str, value: f32, color: Color32) -> (Option<f32>, bool) {
    let resp = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
    let pt = ui.painter();
    let y = rect.center().y;
    let track = Rect::from_min_max(pos2(rect.left(), y - 2.0), pos2(rect.right(), y + 2.0));
    pt.rect_filled(track, 2.0, Color32::from_rgb(52, 63, 80));
    let x = rect.left() + rect.width() * value.clamp(0.0, 1.0);
    pt.rect_filled(Rect::from_min_max(track.min, pos2(x, track.max.y)), 2.0, color);
    let knob = if resp.hovered() || resp.dragged() { 8.0 } else { 7.0 };
    pt.circle_filled(pos2(x, y), knob, color);
    pt.circle_filled(pos2(x, y), knob - 3.5, Color32::from_rgb(230, 240, 255));
    let mut out = None;
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            out = Some(((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
        }
    }
    (out, resp.drag_stopped() || resp.clicked())
}

/// Combo box with a fixed width, styled for the dark theme.
pub fn combo(ui: &mut Ui, rect: Rect, id: &str, selected: &str, add: impl FnOnce(&mut Ui)) {
    region(ui, rect, id, |ui| {
        egui::ComboBox::from_id_salt(id)
            .width(rect.width())
            .height(320.0)
            .selected_text(egui::RichText::new(selected).size(12.5))
            .truncate()
            .show_ui(ui, |ui| {
                ui.set_min_width(rect.width());
                add(ui)
            });
    });
}

pub fn dot(ui: &Ui, center: egui::Pos2, color: Color32) {
    ui.painter().circle_filled(center, 5.0, color);
}

pub fn label(ui: &Ui, pos: egui::Pos2, text: &str, font: FontId, color: Color32) -> Rect {
    ui.painter().text(pos, Align2::LEFT_TOP, text, font, color)
}

pub fn id(ui: &Ui, s: &str) -> Id {
    ui.id().with(s)
}
