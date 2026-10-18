//! Vector icons drawn with lines and shapes, so they look sharp at any size
//! and never show up as empty boxes (the old version relied on missing font symbols).
#![allow(dead_code)]
use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Record,
    Scissors,
    Folder,
    Export,
    Gear,
    Speaker,
    Mic,
    SpeakerMic,
    Muted,
    Refresh,
    Monitor,
    Play,
    Pause,
    Stop,
    Volume,
    Fullscreen,
    Crop,
    Split,
    Trash,
    Wave,
    Film,
    Undo,
    Redo,
    Plus,
    Up,
    Down,
    Close,
    Save,
    Open,
    Pencil,
    Logo,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    // x, y in 0..1 of a square centred in r
    let s = r.width().min(r.height());
    let o = r.center() - vec2(s / 2.0, s / 2.0);
    pos2(o.x + x * s, o.y + y * s)
}

fn line(pt: &Painter, r: Rect, pts: &[(f32, f32)], st: Stroke) {
    pt.add(Shape::line(pts.iter().map(|(x, y)| p(r, *x, *y)).collect(), st));
}

fn poly(pt: &Painter, r: Rect, pts: &[(f32, f32)], fill: Color32) {
    pt.add(Shape::convex_polygon(pts.iter().map(|(x, y)| p(r, *x, *y)).collect(), fill, Stroke::NONE));
}

fn arc(pt: &Painter, c: Pos2, rad: f32, a0: f32, a1: f32, st: Stroke) {
    let n = 24;
    let pts: Vec<Pos2> = (0..=n).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        pos2(c.x + rad * a.cos(), c.y + rad * a.sin())
    }).collect();
    pt.add(Shape::line(pts, st));
}

fn speaker_body(pt: &Painter, r: Rect, c: Color32) {
    poly(pt, r, &[(0.10, 0.38), (0.28, 0.38), (0.50, 0.18), (0.50, 0.82), (0.28, 0.62), (0.10, 0.62)], c);
}

fn mic_body(pt: &Painter, r: Rect, c: Color32, st: Stroke) {
    let s = r.width().min(r.height());
    let cap = Rect::from_min_max(p(r, 0.36, 0.08), p(r, 0.64, 0.58));
    pt.rect_filled(cap, s * 0.14, c);
    arc(pt, p(r, 0.5, 0.42), s * 0.24, 0.0, std::f32::consts::PI, st);
    line(pt, r, &[(0.5, 0.66), (0.5, 0.84)], st);
    line(pt, r, &[(0.36, 0.86), (0.64, 0.86)], st);
}

pub fn draw(pt: &Painter, r: Rect, icon: Icon, c: Color32) {
    let s = r.width().min(r.height());
    let w = (s * 0.09).max(1.4);
    let st = Stroke::new(w, c);
    use std::f32::consts::PI;
    match icon {
        Icon::Record => {
            pt.circle_stroke(r.center(), s * 0.42, Stroke::new(w * 1.1, c));
            pt.circle_filled(r.center(), s * 0.26, c);
        }
        Icon::Scissors => {
            pt.circle_stroke(p(r, 0.26, 0.74), s * 0.14, st);
            pt.circle_stroke(p(r, 0.74, 0.74), s * 0.14, st);
            line(pt, r, &[(0.34, 0.63), (0.80, 0.12)], st);
            line(pt, r, &[(0.66, 0.63), (0.20, 0.12)], st);
        }
        Icon::Folder | Icon::Open => {
            let body = Rect::from_min_max(p(r, 0.08, 0.30), p(r, 0.92, 0.84));
            pt.rect_filled(body, s * 0.06, c);
            poly(pt, r, &[(0.08, 0.34), (0.08, 0.20), (0.40, 0.20), (0.48, 0.30), (0.48, 0.34)], c);
            if icon == Icon::Folder {
                line(pt, r, &[(0.14, 0.42), (0.86, 0.42)], Stroke::new(w * 0.8, Color32::from_black_alpha(90)));
            }
        }
        Icon::Export => {
            line(pt, r, &[(0.40, 0.18), (0.14, 0.18), (0.14, 0.86), (0.82, 0.86), (0.82, 0.60)], st);
            line(pt, r, &[(0.44, 0.56), (0.86, 0.14)], st);
            line(pt, r, &[(0.56, 0.14), (0.86, 0.14), (0.86, 0.44)], st);
        }
        Icon::Gear => {
            let center = r.center();
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                let d = vec2(a.cos(), a.sin());
                pt.line_segment([center + d * s * 0.28, center + d * s * 0.44], Stroke::new(s * 0.14, c));
            }
            pt.circle_filled(center, s * 0.32, c);
            pt.circle_filled(center, s * 0.13, Color32::from_rgb(20, 28, 39));
        }
        Icon::Speaker => {
            speaker_body(pt, r, c);
            arc(pt, p(r, 0.52, 0.5), s * 0.16, -0.9, 0.9, st);
            arc(pt, p(r, 0.52, 0.5), s * 0.30, -0.9, 0.9, st);
        }
        Icon::Volume => {
            speaker_body(pt, r, c);
            arc(pt, p(r, 0.52, 0.5), s * 0.16, -0.9, 0.9, st);
            arc(pt, p(r, 0.52, 0.5), s * 0.30, -0.9, 0.9, st);
        }
        Icon::Muted => {
            speaker_body(pt, r, c);
            line(pt, r, &[(0.62, 0.36), (0.88, 0.64)], st);
            line(pt, r, &[(0.88, 0.36), (0.62, 0.64)], st);
            line(pt, r, &[(0.06, 0.12), (0.92, 0.90)], st);
        }
        Icon::Mic => mic_body(pt, r, c, st),
        Icon::SpeakerMic => {
            let left = Rect::from_min_size(r.min, vec2(r.width() * 0.58, r.height()));
            let right = Rect::from_min_max(pos2(r.min.x + r.width() * 0.48, r.min.y), r.max);
            speaker_body(pt, left, c);
            let ls = Stroke::new(w * 0.9, c);
            arc(pt, p(left, 0.52, 0.5), left.width().min(left.height()) * 0.24, -0.9, 0.9, ls);
            mic_body(pt, right, c, ls);
        }
        Icon::Refresh => {
            arc(pt, r.center(), s * 0.32, -PI * 0.15, PI * 1.45, st);
            let tip = r.center() + vec2((-PI * 0.15).cos(), (-PI * 0.15).sin()) * s * 0.32;
            pt.add(Shape::convex_polygon(vec![tip + vec2(-s * 0.16, -s * 0.06), tip + vec2(s * 0.10, -s * 0.14), tip + vec2(s * 0.06, s * 0.12)], c, Stroke::NONE));
        }
        Icon::Monitor => {
            let scr = Rect::from_min_max(p(r, 0.06, 0.14), p(r, 0.94, 0.70));
            pt.rect_stroke(scr, s * 0.05, st, eframe::egui::StrokeKind::Middle);
            line(pt, r, &[(0.5, 0.70), (0.5, 0.84)], st);
            line(pt, r, &[(0.30, 0.86), (0.70, 0.86)], st);
        }
        Icon::Play => poly(pt, r, &[(0.26, 0.14), (0.86, 0.5), (0.26, 0.86)], c),
        Icon::Pause => {
            pt.rect_filled(Rect::from_min_max(p(r, 0.22, 0.14), p(r, 0.42, 0.86)), s * 0.03, c);
            pt.rect_filled(Rect::from_min_max(p(r, 0.58, 0.14), p(r, 0.78, 0.86)), s * 0.03, c);
        }
        Icon::Stop => {
            pt.rect_filled(Rect::from_min_max(p(r, 0.2, 0.2), p(r, 0.8, 0.8)), s * 0.06, c);
        }
        Icon::Fullscreen => {
            for (a, b, cc) in [((0.12, 0.38), (0.12, 0.12), (0.38, 0.12)), ((0.62, 0.12), (0.88, 0.12), (0.88, 0.38)), ((0.88, 0.62), (0.88, 0.88), (0.62, 0.88)), ((0.38, 0.88), (0.12, 0.88), (0.12, 0.62))] {
                line(pt, r, &[a, b, cc], st);
            }
        }
        Icon::Crop => {
            line(pt, r, &[(0.26, 0.06), (0.26, 0.74), (0.94, 0.74)], st);
            line(pt, r, &[(0.06, 0.26), (0.74, 0.26), (0.74, 0.94)], st);
        }
        Icon::Split => {
            pt.rect_stroke(Rect::from_min_max(p(r, 0.08, 0.20), p(r, 0.42, 0.80)), s * 0.04, st, eframe::egui::StrokeKind::Middle);
            pt.rect_stroke(Rect::from_min_max(p(r, 0.58, 0.20), p(r, 0.92, 0.80)), s * 0.04, st, eframe::egui::StrokeKind::Middle);
            line(pt, r, &[(0.5, 0.06), (0.5, 0.94)], st);
        }
        Icon::Trash => {
            line(pt, r, &[(0.14, 0.24), (0.86, 0.24)], st);
            line(pt, r, &[(0.38, 0.24), (0.40, 0.12), (0.60, 0.12), (0.62, 0.24)], st);
            line(pt, r, &[(0.22, 0.24), (0.28, 0.88), (0.72, 0.88), (0.78, 0.24)], st);
            line(pt, r, &[(0.42, 0.40), (0.43, 0.74)], st);
            line(pt, r, &[(0.58, 0.40), (0.57, 0.74)], st);
        }
        Icon::Wave => {
            let hs = [0.18, 0.42, 0.70, 0.30, 0.86, 0.50, 0.24];
            for (i, h) in hs.iter().enumerate() {
                let x = 0.14 + i as f32 * 0.12;
                line(pt, r, &[(x, 0.5 - h / 2.0), (x, 0.5 + h / 2.0)], st);
            }
        }
        Icon::Film => {
            let body = Rect::from_min_max(p(r, 0.08, 0.16), p(r, 0.92, 0.84));
            pt.rect_stroke(body, s * 0.05, st, eframe::egui::StrokeKind::Middle);
            line(pt, r, &[(0.30, 0.16), (0.30, 0.84)], st);
            line(pt, r, &[(0.70, 0.16), (0.70, 0.84)], st);
            for y in [0.30, 0.5, 0.70] {
                line(pt, r, &[(0.08, y), (0.30, y)], Stroke::new(w * 0.7, c));
                line(pt, r, &[(0.70, y), (0.92, y)], Stroke::new(w * 0.7, c));
            }
        }
        Icon::Undo | Icon::Redo => {
            let flip = icon == Icon::Redo;
            let fx = |x: f32| if flip { 1.0 - x } else { x };
            line(pt, r, &[(fx(0.20), 0.36), (fx(0.62), 0.36)], st);
            arc_open(pt, r, flip, st);
            line(pt, r, &[(fx(0.34), 0.20), (fx(0.18), 0.36), (fx(0.34), 0.52)], st);
        }
        Icon::Plus => {
            line(pt, r, &[(0.5, 0.16), (0.5, 0.84)], st);
            line(pt, r, &[(0.16, 0.5), (0.84, 0.5)], st);
        }
        Icon::Up => line(pt, r, &[(0.2, 0.66), (0.5, 0.34), (0.8, 0.66)], st),
        Icon::Down => line(pt, r, &[(0.2, 0.34), (0.5, 0.66), (0.8, 0.34)], st),
        Icon::Close => {
            line(pt, r, &[(0.2, 0.2), (0.8, 0.8)], st);
            line(pt, r, &[(0.8, 0.2), (0.2, 0.8)], st);
        }
        Icon::Save => {
            line(pt, r, &[(0.14, 0.12), (0.72, 0.12), (0.88, 0.28), (0.88, 0.88), (0.14, 0.88), (0.14, 0.12)], st);
            pt.rect_filled(Rect::from_min_max(p(r, 0.30, 0.12), p(r, 0.64, 0.34)), 0.0, c);
            pt.rect_stroke(Rect::from_min_max(p(r, 0.28, 0.56), p(r, 0.72, 0.88)), 0.0, st, eframe::egui::StrokeKind::Middle);
        }
        Icon::Pencil => {
            line(pt, r, &[(0.18, 0.82), (0.22, 0.62), (0.68, 0.16), (0.84, 0.32), (0.38, 0.78), (0.18, 0.82)], st);
            line(pt, r, &[(0.58, 0.26), (0.74, 0.42)], st);
        }
        Icon::Logo => logo(pt, r),
    }
}

fn arc_open(pt: &Painter, r: Rect, flip: bool, st: Stroke) {
    // curved tail of the undo/redo arrow
    let s = r.width().min(r.height());
    let c = p(r, if flip { 0.38 } else { 0.62 }, 0.58);
    let (a0, a1) = if flip { (std::f32::consts::PI * 1.5, std::f32::consts::PI * 0.6) } else { (-std::f32::consts::PI * 0.5, std::f32::consts::PI * 0.4) };
    let n = 18;
    let pts: Vec<Pos2> = (0..=n).map(|i| {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        pos2(c.x + s * 0.22 * a.cos(), c.y + s * 0.22 * a.sin())
    }).collect();
    pt.add(Shape::line(pts, st));
}

/// The camera-aperture logo from the design.
pub fn logo(pt: &Painter, r: Rect) {
    let s = r.width().min(r.height());
    let c = r.center();
    let blue = Color32::from_rgb(70, 170, 245);
    let light = Color32::from_rgb(210, 236, 255);
    pt.circle_filled(c, s * 0.48, Color32::from_rgb(16, 34, 56));
    pt.circle_stroke(c, s * 0.46, Stroke::new(s * 0.06, blue));
    let n = 6;
    for i in 0..n {
        let a0 = i as f32 * std::f32::consts::TAU / n as f32;
        let a1 = a0 + std::f32::consts::TAU / n as f32;
        let o0 = c + vec2(a0.cos(), a0.sin()) * s * 0.38;
        let o1 = c + vec2(a1.cos(), a1.sin()) * s * 0.38;
        let inner = c + vec2((a0 + 0.9).cos(), (a0 + 0.9).sin()) * s * 0.15;
        let col = if i % 2 == 0 { blue } else { light };
        pt.add(Shape::convex_polygon(vec![o0, o1, inner], col, Stroke::new(s * 0.012, Color32::from_rgb(16, 34, 56))));
    }
    pt.circle_filled(c, s * 0.12, Color32::from_rgb(16, 34, 56));
}

/// Builds the window icon (RGBA) by drawing the logo shape in software.
pub fn window_icon(size: u32) -> Vec<u8> {
    let mut px = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let d = (dx * dx + dy * dy).sqrt() / c;
            let a = dy.atan2(dx).rem_euclid(std::f32::consts::TAU);
            let seg = (a / (std::f32::consts::TAU / 6.0)) as i32;
            let (r, g, b, al) = if d > 0.98 {
                (0, 0, 0, 0)
            } else if d > 0.84 {
                (70, 170, 245, 255)
            } else if d > 0.26 {
                if seg % 2 == 0 { (70, 170, 245, 255) } else { (210, 236, 255, 255) }
            } else {
                (16, 34, 56, 255)
            };
            let i = ((y * size + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[r, g, b, al]);
        }
    }
    px
}
