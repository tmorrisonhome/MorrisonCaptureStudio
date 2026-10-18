//! Main screen layout: header, recording panels, tabs, file list, preview, waveform, tools, status bar.
use crate::app::{Dialog, Page, StatusKind, Studio, Tab};
use crate::capture::{AudioMode, QUALITIES};
use crate::edit::{clock, FORMATS, QUALITY};
use crate::icons::{self, Icon};
use crate::library::{hms, size_text};
use crate::theme::{self, *};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

pub fn draw(app: &mut Studio, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, BG);
    shortcuts(app, &ctx);
    if app.fullscreen {
        fullscreen(app, ui, full);
        crate::pages::dialogs(app, &ctx);
        return;
    }
    let header_r = Rect::from_min_size(full.min, vec2(full.width(), 80.0));
    let status_r = Rect::from_min_max(pos2(full.left(), full.bottom() - 30.0), full.max);
    let body = Rect::from_min_max(pos2(full.left() + 12.0, header_r.bottom() + 10.0), pos2(full.right() - 12.0, status_r.top() - 10.0));
    header(app, ui, header_r);
    match app.page {
        Page::Record => record_page(app, ui, body),
        Page::Timeline => crate::pages::timeline_page(app, ui, body),
        Page::Library => crate::pages::library_page(app, ui, body),
        Page::Export => crate::pages::export_page(app, ui, body),
        Page::Settings => crate::pages::settings_page(app, ui, body),
    }
    status_bar(app, ui, status_r);
    crate::pages::dialogs(app, &ctx);
}

fn shortcuts(app: &mut Studio, ctx: &egui::Context) {
    use egui::{Key, KeyboardShortcut, Modifiers};
    let typing = ctx.egui_wants_keyboard_input();
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z))) {
        app.undo();
    }
    if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y))) {
        app.redo();
    }
    if !typing && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Space)) && app.dialog == Dialog::None {
        app.toggle_play(ctx);
    }
    if app.fullscreen && ctx.input(|i| i.key_pressed(Key::Escape)) {
        set_fullscreen(app, ctx, false);
    }
}

pub fn set_fullscreen(app: &mut Studio, ctx: &egui::Context, on: bool) {
    app.fullscreen = on;
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
}

// ---------------------------------------------------------------- header
fn header(app: &mut Studio, ui: &mut Ui, r: Rect) {
    let pt = ui.painter();
    pt.rect_filled(r, 0.0, HEADER);
    pt.line_segment([pos2(r.left(), r.bottom()), pos2(r.right(), r.bottom())], Stroke::new(1.0, PANEL_BORDER));
    icons::logo(pt, Rect::from_center_size(pos2(r.left() + 44.0, r.center().y), vec2(54.0, 54.0)));
    let tx = r.left() + 82.0;
    let a = pt.text(pos2(tx, r.top() + 13.0), Align2::LEFT_TOP, "MORRISON", bold(31.0), Color32::WHITE);
    let b = pt.text(pos2(tx + 1.0, a.bottom() - 4.0), Align2::LEFT_TOP, "CAPTURE STUDIO", bold(20.5), BLUE_LIGHT);
    let sx = a.right().max(b.right()) + 28.0;
    for (i, line) in ["Rust Edition", "Audio + Screen Recording", "Editing + Media Export"].iter().enumerate() {
        pt.text(pos2(sx, r.top() + 17.0 + i as f32 * 16.0), Align2::LEFT_TOP, *line, reg(12.5), Color32::from_rgb(200, 210, 222));
    }
    let nav_left = (sx + 190.0).max(r.left() + 470.0);
    let items: [(Page, &str, Icon, f32); 5] = [
        (Page::Record, "Record", Icon::Record, 96.0),
        (Page::Timeline, "Edit / Timeline", Icon::Scissors, 132.0),
        (Page::Library, "Media Library", Icon::Folder, 130.0),
        (Page::Export, "Export", Icon::Export, 98.0),
        (Page::Settings, "Settings", Icon::Gear, 110.0),
    ];
    let gap = 8.0;
    let avail = r.right() - 14.0 - nav_left - gap * 4.0;
    let total: f32 = items.iter().map(|i| i.3).sum();
    let scale = (avail / total).clamp(0.75, 1.35);
    let mut x = r.right() - 14.0 - (total * scale + gap * 4.0);
    for (page, label, icon, w) in items {
        let br = Rect::from_min_size(pos2(x, r.top() + 14.0), vec2(w * scale, r.height() - 28.0));
        x += w * scale + gap;
        let resp = ui.interact(br, ui.id().with(("nav", label)), Sense::click());
        let active = app.page == page;
        let fill = if active { Color32::from_rgb(30, 48, 76) } else if resp.hovered() { FIELD_HOVER } else { Color32::from_rgb(22, 31, 44) };
        let stroke = if active { Stroke::new(1.5, BLUE) } else { Stroke::new(1.0, FIELD_BORDER) };
        let pt = ui.painter();
        pt.rect_filled(br, 6.0, fill);
        pt.rect_stroke(br, 6.0, stroke, StrokeKind::Inside);
        let galley = pt.layout_no_wrap(label.to_string(), semi(14.0), TEXT);
        let isz = 18.0;
        let tw = isz + 10.0 + galley.size().x;
        let ix = br.center().x - tw / 2.0;
        let ir = Rect::from_min_size(pos2(ix, br.center().y - isz / 2.0), vec2(isz, isz));
        if icon == Icon::Record {
            pt.circle_filled(ir.center(), isz * 0.48, Color32::WHITE);
            pt.circle_filled(ir.center(), isz * 0.38, RED);
        } else {
            icons::draw(pt, ir, icon, TEXT);
        }
        pt.galley(pos2(ix + isz + 10.0, br.center().y - galley.size().y / 2.0), galley, TEXT);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            app.page = page;
        }
    }
}

// ---------------------------------------------------------------- record page
fn record_page(app: &mut Studio, ui: &mut Ui, body: Rect) {
    let ctx = ui.ctx().clone();
    let row_h = 210.0;
    let row = Rect::from_min_size(body.min, vec2(body.width(), row_h));
    let weights = [344.0, 257.0, 214.0, 204.0];
    let gap = 8.0;
    let unit = (row.width() - gap * 3.0) / weights.iter().sum::<f32>();
    let mut x = row.left();
    let mut cols = Vec::new();
    for w in weights {
        cols.push(Rect::from_min_size(pos2(x, row.top()), vec2(w * unit, row_h)));
        x += w * unit + gap;
    }
    what_to_record(app, ui, cols[0]);
    audio_devices(app, ui, cols[1], &ctx);
    recording_options(app, ui, cols[2]);
    start_panel(app, ui, cols[3], &ctx);

    // Tabs
    let tabs_r = Rect::from_min_size(pos2(body.left(), row.bottom() + 10.0), vec2(body.width(), 36.0));
    let tabs = [(Tab::Recordings, "Recordings", Icon::Folder), (Tab::AudioEditor, "Audio Editor", Icon::Wave), (Tab::VideoEditor, "Video Editor", Icon::Film)];
    let mut tx = tabs_r.left();
    for (tab, label, icon) in tabs {
        let w = 124.0;
        let tr = Rect::from_min_size(pos2(tx, tabs_r.top()), vec2(w, tabs_r.height()));
        tx += w + 2.0;
        let resp = ui.interact(tr, ui.id().with(("tab", label)), Sense::click());
        let active = app.tab == tab;
        let pt = ui.painter();
        let fill = if active { Color32::from_rgb(33, 62, 106) } else if resp.hovered() { FIELD_HOVER } else { Color32::from_rgb(20, 28, 40) };
        pt.rect_filled(tr, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, fill);
        let col = if active { Color32::WHITE } else { DIM };
        let g = pt.layout_no_wrap(label.to_string(), semi(13.5), col);
        let tw = 16.0 + 8.0 + g.size().x;
        let ix = tr.center().x - tw / 2.0;
        icons::draw(pt, Rect::from_min_size(pos2(ix, tr.center().y - 8.0), vec2(16.0, 16.0)), icon, col);
        pt.galley(pos2(ix + 24.0, tr.center().y - g.size().y / 2.0), g, col);
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            app.tab = tab;
        }
    }
    ui.painter().line_segment([pos2(body.left(), tabs_r.bottom()), pos2(body.right(), tabs_r.bottom())], Stroke::new(1.0, PANEL_BORDER));

    let content = Rect::from_min_max(pos2(body.left(), tabs_r.bottom() + 6.0), body.max);
    match app.tab {
        Tab::Recordings => recordings_tab(app, ui, content),
        Tab::AudioEditor => crate::pages::audio_editor_tab(app, ui, content),
        Tab::VideoEditor => crate::pages::video_editor_tab(app, ui, content),
    }
}

fn what_to_record(app: &mut Studio, ui: &mut Ui, r: Rect) {
    let inner = theme::panel(ui, r, "1. Select What to Record");
    let enabled = !app.recording();
    let gap = 8.0;
    let tw = (inner.width() - gap * 3.0) / 4.0;
    let th = (inner.height() - 6.0).min(tw * 1.42);
    let opts = [
        (AudioMode::Speakers, Icon::Speaker, "Speakers\nOnly"),
        (AudioMode::Microphone, Icon::Mic, "Microphone\nOnly"),
        (AudioMode::Both, Icon::SpeakerMic, "Speakers +\nMicrophone"),
        (AudioMode::Off, Icon::Muted, "No Audio\n(Screen Only)"),
    ];
    for (i, (mode, icon, label)) in opts.into_iter().enumerate() {
        let tr = Rect::from_min_size(pos2(inner.left() + i as f32 * (tw + gap), inner.top() + 2.0), vec2(tw, th));
        if theme::tile(ui, tr, label, icon, label, app.mode == mode, enabled).clicked() {
            app.mode = mode;
            if mode == AudioMode::Off {
                app.settings.record_screen = true;
            }
            app.settings.audio_mode = mode.to_u8();
            app.settings.save();
        }
    }
}

fn audio_devices(app: &mut Studio, ui: &mut Ui, r: Rect, ctx: &egui::Context) {
    let inner = theme::panel(ui, r, "2. Select Audio Devices");
    let enabled = !app.recording();
    let ix = inner.left() + 32.0;
    let cw = inner.right() - ix;
    // Speaker row
    icons::draw(ui.painter(), Rect::from_min_size(pos2(inner.left(), inner.top() + 14.0), vec2(22.0, 22.0)), Icon::Speaker, DIM);
    label(ui, pos2(ix, inner.top()), "Speaker / Output Device", reg(12.5), DIM);
    let sp_label = app.speaker_label();
    ui.add_enabled_ui(enabled, |ui| {
        let speakers = app.devices.speakers.clone();
        let def = app.devices.default_speaker.clone();
        theme::combo(ui, Rect::from_min_size(pos2(ix, inner.top() + 19.0), vec2(cw, 26.0)), "speaker-dev", &sp_label, |ui| {
            let d = if def.is_empty() { "Default speaker".to_string() } else { format!("Default speaker ({def})") };
            ui.selectable_value(&mut app.speaker_sel, None, d);
            for (i, dv) in speakers.iter().enumerate() {
                ui.selectable_value(&mut app.speaker_sel, Some(i), &dv.name);
            }
        });
    });
    // Mic row
    let my = inner.top() + 58.0;
    icons::draw(ui.painter(), Rect::from_min_size(pos2(inner.left(), my + 14.0), vec2(22.0, 22.0)), Icon::Mic, DIM);
    label(ui, pos2(ix, my), "Microphone Device", reg(12.5), DIM);
    let mic_label = app.mic_label();
    ui.add_enabled_ui(enabled, |ui| {
        let mics = app.devices.microphones.clone();
        let def = app.devices.default_microphone.clone();
        theme::combo(ui, Rect::from_min_size(pos2(ix, my + 19.0), vec2(cw, 26.0)), "mic-dev", &mic_label, |ui| {
            let d = if def.is_empty() { "Default microphone".to_string() } else { format!("Default microphone ({def})") };
            ui.selectable_value(&mut app.mic_sel, None, d);
            for (i, dv) in mics.iter().enumerate() {
                ui.selectable_value(&mut app.mic_sel, Some(i), &dv.name);
            }
        });
    });
    let br = Rect::from_min_size(pos2(inner.left(), inner.bottom() - 32.0), vec2(170.0f32.min(inner.width()), 32.0));
    let refreshing = app.dev_rx.is_some();
    if theme::button(ui, br, "refresh-dev", if refreshing { "Refreshing..." } else { "Refresh Audio Devices" }, Some(Icon::Refresh), Kind::Normal, enabled && !refreshing, 12.5).clicked() {
        app.refresh_devices(ctx);
        app.refresh_screens();
    }
}

fn recording_options(app: &mut Studio, ui: &mut Ui, r: Rect) {
    let inner = theme::panel(ui, r, "3. Recording Options");
    let en = !app.recording();
    let rh = 25.0;
    let mut y = inner.top() - 4.0;
    // Record screen
    icons::draw(ui.painter(), Rect::from_min_size(pos2(inner.left(), y + 4.0), vec2(18.0, 18.0)), Icon::Monitor, TEXT);
    let rec_label = if inner.width() >= 205.0 { "Record Desktop / Screen" } else { "Record Screen" };
    label(ui, pos2(inner.left() + 26.0, y + 4.0), rec_label, reg(13.0), TEXT);
    let mut scr = app.settings.record_screen;
    if theme::toggle(ui, Rect::from_min_size(pos2(inner.right() - 34.0, y + 4.0), vec2(34.0, 18.0)), "rec-screen", &mut scr, en && app.mode != AudioMode::Off) {
        app.settings.record_screen = scr;
        app.settings.save();
    }
    y += rh + 2.0;
    let screen_on = app.settings.record_screen;
    let sl = app.screen_label();
    let screens: Vec<String> = app.screens.iter().map(|s| s.label.clone()).collect();
    let mut sel = app.screen_sel;
    ui.add_enabled_ui(en && screen_on, |ui| {
        theme::combo(ui, Rect::from_min_size(pos2(inner.left() + 26.0, y), vec2(inner.width() - 26.0, 24.0)), "screen-sel", &sl, |ui| {
            for (i, s) in screens.iter().enumerate() {
                ui.selectable_value(&mut sel, i, s);
            }
        });
    });
    if sel != app.screen_sel {
        app.screen_sel = sel;
        if app.screens.get(sel).map(|s| s.rect.is_none()).unwrap_or(false) {
            app.dialog = Dialog::Region;
        }
    }
    y += rh + 4.0;
    let mut cur = app.settings.record_cursor;
    if theme::toggle_row(ui, Rect::from_min_size(pos2(inner.left() + 26.0, y), vec2(inner.width() - 26.0, 20.0)), "rec-cursor", "Record Cursor", &mut cur, en && screen_on) {
        app.settings.record_cursor = cur;
        app.settings.save();
    }
    y += rh - 2.0;
    let mut hl = app.settings.highlight_clicks;
    if theme::toggle_row(ui, Rect::from_min_size(pos2(inner.left() + 26.0, y), vec2(inner.width() - 26.0, 20.0)), "rec-clicks", "Highlight Clicks", &mut hl, en && screen_on) {
        app.settings.highlight_clicks = hl;
        app.settings.save();
    }
    y += rh + 2.0;
    let cx = inner.left() + 90.0;
    let cw = inner.right() - cx;
    label(ui, pos2(inner.left(), y + 4.0), "Video Quality:", reg(12.5), TEXT);
    let mut q = app.settings.video_quality;
    ui.add_enabled_ui(en && screen_on, |ui| {
        theme::combo(ui, Rect::from_min_size(pos2(cx, y), vec2(cw, 24.0)), "vid-q", QUALITIES[q.min(3)].0, |ui| {
            for (i, (l, _, _)) in QUALITIES.iter().enumerate() {
                ui.selectable_value(&mut q, i, *l);
            }
        });
    });
    y += rh + 4.0;
    label(ui, pos2(inner.left(), y + 4.0), "Frame Rate:", reg(12.5), TEXT);
    let mut fps = app.settings.frame_rate;
    ui.add_enabled_ui(en && screen_on, |ui| {
        theme::combo(ui, Rect::from_min_size(pos2(cx, y), vec2(cw, 24.0)), "fps", &format!("{fps} FPS"), |ui| {
            for f in [15, 24, 30, 60] {
                ui.selectable_value(&mut fps, f, format!("{f} FPS"));
            }
        });
    });
    if q != app.settings.video_quality || fps != app.settings.frame_rate {
        app.settings.video_quality = q;
        app.settings.frame_rate = fps;
        app.settings.save();
    }
}

fn start_panel(app: &mut Studio, ui: &mut Ui, r: Rect, ctx: &egui::Context) {
    let inner = theme::panel(ui, r, "4. Start Recording");
    let rec = app.recording();
    let saving = app.job.as_ref().map(|j| j.label == "Saving recording").unwrap_or(false);
    let b1 = Rect::from_min_size(inner.min, vec2(inner.width(), 46.0));
    let label1 = if rec { "RECORDING..." } else { "START RECORDING" };
    if theme::button(ui, b1, "start-rec", label1, Some(Icon::Record), Kind::Danger, !rec && !saving, 15.5).clicked() {
        app.start_recording();
    }
    let b2 = Rect::from_min_size(pos2(inner.left(), b1.bottom() + 10.0), vec2(inner.width(), 40.0));
    if theme::button(ui, b2, "stop-rec", "STOP & SAVE", Some(Icon::Stop), if rec { Kind::Primary } else { Kind::Normal }, rec, 14.0).clicked() {
        app.stop_recording(ctx);
    }
    let secs = app.record_seconds();
    let timer = format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60);
    let pt = ui.painter();
    let tr = pt.text(pos2(inner.center().x, b2.bottom() + 10.0), Align2::CENTER_TOP, timer, bold(27.0), Color32::WHITE);
    let (col, txt) = if rec {
        let blink = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
        (if blink { RED } else { Color32::from_rgb(120, 30, 34) }, "Recording")
    } else if saving {
        (ORANGE, "Saving...")
    } else {
        (GREEN, "Ready to record")
    };
    let g = pt.layout_no_wrap(txt.to_string(), reg(13.0), TEXT);
    let w = 16.0 + g.size().x;
    let sx = inner.center().x - w / 2.0;
    let cy = tr.bottom() + 12.0;
    pt.circle_filled(pos2(sx + 5.0, cy), 5.5, col);
    pt.galley(pos2(sx + 16.0, cy - g.size().y / 2.0), g, TEXT);
}

// ---------------------------------------------------------------- recordings tab
pub fn columns(content: Rect) -> (Rect, Rect, Rect) {
    let gap = 8.0;
    let weights = [254.0, 380.0, 392.0];
    let unit = (content.width() - gap * 2.0) / weights.iter().sum::<f32>();
    let a = Rect::from_min_size(content.min, vec2(weights[0] * unit, content.height()));
    let b = Rect::from_min_size(pos2(a.right() + gap, content.top()), vec2(weights[1] * unit, content.height()));
    let c = Rect::from_min_max(pos2(b.right() + gap, content.top()), content.max);
    (a, b, c)
}

fn recordings_tab(app: &mut Studio, ui: &mut Ui, content: Rect) {
    let ctx = ui.ctx().clone();
    let (left, mid, right) = columns(content);
    file_list(app, ui, left);
    let tools_h = 96.0;
    let gap = 8.0;
    let prev_r = Rect::from_min_max(mid.min, pos2(mid.right(), mid.bottom() - tools_h - gap));
    let tools_r = Rect::from_min_max(pos2(mid.left(), prev_r.bottom() + gap), mid.max);
    let wave_r = Rect::from_min_max(right.min, pos2(right.right(), right.bottom() - tools_h - gap));
    let export_r = Rect::from_min_max(pos2(right.left(), wave_r.bottom() + gap), right.max);

    // Preview
    let inner = theme::panel(ui, prev_r, "Preview");
    if theme::icon_button(ui, Rect::from_min_size(pos2(prev_r.right() - 34.0, prev_r.top() + 7.0), vec2(26.0, 26.0)), "prev-gear", Icon::Gear, "Preview settings", true).clicked() {
        app.dialog = Dialog::PreviewSettings;
    }
    let video_r = Rect::from_min_max(inner.min, pos2(inner.right(), inner.bottom() - 36.0));
    preview_area(app, ui, video_r, false);
    transport(app, ui, Rect::from_min_max(pos2(inner.left(), video_r.bottom() + 4.0), inner.max), &ctx);

    // Waveform
    let winner = theme::panel(ui, wave_r, "Audio Waveform (Selected File)");
    waveform_view(app, ui, winner, &ctx);

    // Quick tools
    let tinner = theme::panel(ui, tools_r, "Quick Tools");
    let tools = [(Icon::Scissors, "Trim"), (Icon::Volume, "Volume"), (Icon::Crop, "Crop"), (Icon::Split, "Split"), (Icon::Trash, "Delete")];
    let g = 8.0;
    let bw = ((tinner.width() - g * 4.0) / 5.0).min(78.0);
    let has = app.selected.is_some() && app.info.is_some();
    let is_video = app.info.as_ref().map(|i| i.has_video).unwrap_or(false);
    for (i, (icon, name)) in tools.iter().enumerate() {
        let br = Rect::from_min_size(pos2(tinner.left() + i as f32 * (bw + g), tinner.top() - 4.0), vec2(bw, tinner.height() + 4.0));
        let enabled = has && (*name != "Crop" || is_video) && !app.recording();
        if tool_button(ui, br, name, *icon, enabled).clicked() {
            match *name {
                "Trim" => app.dialog = Dialog::Trim,
                "Volume" => app.dialog = Dialog::Volume,
                "Crop" => {
                    app.crop_mode = true;
                    app.dialog = Dialog::Crop;
                }
                "Split" => app.split(&ctx),
                _ => {
                    if let Some(p) = app.selected.clone() {
                        app.dialog = Dialog::Delete(p);
                    }
                }
            }
        }
    }

    // Export
    export_panel(app, ui, export_r, &ctx);
}

fn tool_button(ui: &Ui, r: Rect, name: &str, icon: Icon, enabled: bool) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("tool", name)), if enabled { Sense::click() } else { Sense::hover() });
    let pt = ui.painter();
    let fill = if enabled && resp.hovered() { FIELD_HOVER } else { Color32::from_rgb(28, 38, 52) };
    pt.rect_filled(r, 6.0, fill);
    pt.rect_stroke(r, 6.0, Stroke::new(1.0, FIELD_BORDER), StrokeKind::Inside);
    let c = if enabled { TEXT } else { Color32::from_rgb(105, 116, 132) };
    icons::draw(pt, Rect::from_center_size(pos2(r.center().x, r.top() + r.height() * 0.36), vec2(20.0, 20.0)), icon, c);
    pt.text(pos2(r.center().x, r.top() + r.height() * 0.74), Align2::CENTER_CENTER, name, reg(12.5), c);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

fn export_panel(app: &mut Studio, ui: &mut Ui, r: Rect, ctx: &egui::Context) {
    let inner = theme::panel(ui, r, "Export");
    let right_w = (inner.width() * 0.40).clamp(140.0, 190.0);
    let left = Rect::from_min_max(inner.min, pos2(inner.right() - right_w - 14.0, inner.bottom()));
    let lw = 58.0;
    let combo_w = left.width() - lw - 34.0;
    let y1 = left.top() - 6.0;
    label(ui, pos2(left.left(), y1 + 4.0), "Format:", reg(12.5), TEXT);
    let mut f = app.settings.export_format.min(FORMATS.len() - 1);
    theme::combo(ui, Rect::from_min_size(pos2(left.left() + lw, y1), vec2(combo_w, 24.0)), "exp-fmt", FORMATS[f].label, |ui| {
        for (i, fm) in FORMATS.iter().enumerate() {
            ui.selectable_value(&mut f, i, fm.label);
        }
    });
    if theme::icon_button(ui, Rect::from_min_size(pos2(left.right() - 26.0, y1 - 1.0), vec2(26.0, 26.0)), "exp-gear", Icon::Gear, "All export settings", true).clicked() {
        app.page = Page::Export;
    }
    let y2 = y1 + 32.0;
    label(ui, pos2(left.left(), y2 + 4.0), "Quality:", reg(12.5), TEXT);
    let mut q = app.settings.export_quality.min(QUALITY.len() - 1);
    theme::combo(ui, Rect::from_min_size(pos2(left.left() + lw, y2), vec2(combo_w, 24.0)), "exp-q", QUALITY[q], |ui| {
        for (i, l) in QUALITY.iter().enumerate() {
            ui.selectable_value(&mut q, i, *l);
        }
    });
    if f != app.settings.export_format || q != app.settings.export_quality {
        app.settings.export_format = f;
        app.settings.export_quality = q;
        app.settings.save();
    }
    let rx = inner.right() - right_w;
    let b1 = Rect::from_min_size(pos2(rx, inner.top() - 8.0), vec2(right_w, 34.0));
    let can = app.selected.is_some() && app.info.is_some() && !app.busy() && !app.recording();
    if theme::button(ui, b1, "export-sel", "Export Selected", Some(Icon::Export), Kind::Primary, can, 14.0).clicked() {
        app.export_selected(ctx);
    }
    let b2 = Rect::from_min_size(pos2(rx, b1.bottom() + 8.0), vec2(right_w, 30.0));
    if theme::button(ui, b2, "open-out", "Open Output Folder", Some(Icon::Open), Kind::Outline, true, 12.5).clicked() {
        app.open_folder(&app.settings.output_dir.clone());
    }
}

// ---------------------------------------------------------------- file list
pub fn file_list(app: &mut Studio, ui: &mut Ui, r: Rect) {
    let ctx = ui.ctx().clone();
    let title = format!("Recorded Files ({})", app.library.entries.len());
    let inner = theme::panel(ui, r, &title);
    let by = r.top() + 7.0;
    if theme::icon_button(ui, Rect::from_min_size(pos2(r.right() - 68.0, by), vec2(26.0, 26.0)), "lib-open", Icon::Open, "Open the output folder", true).clicked() {
        app.open_folder(&app.settings.output_dir.clone());
    }
    let can_del = app.selected.as_ref().map(|p| p.parent() == Some(app.settings.output_dir.as_path())).unwrap_or(false);
    if theme::icon_button(ui, Rect::from_min_size(pos2(r.right() - 36.0, by), vec2(26.0, 26.0)), "lib-del", Icon::Trash, "Delete the selected file (to Recycle Bin)", can_del).clicked() {
        if let Some(p) = app.selected.clone() {
            app.dialog = Dialog::Delete(p);
        }
    }
    let list_r = Rect::from_min_max(pos2(r.left() + 4.0, inner.top() - 2.0), pos2(r.right() - 4.0, r.bottom() - 6.0));
    if app.library.entries.is_empty() {
        let pt = ui.painter();
        pt.text(pos2(list_r.center().x, list_r.top() + 40.0), Align2::CENTER_CENTER, "No recordings yet", semi(14.0), DIM);
        pt.text(pos2(list_r.center().x, list_r.top() + 62.0), Align2::CENTER_CENTER, "Press START RECORDING above", reg(12.0), DIM);
        let br = Rect::from_center_size(pos2(list_r.center().x, list_r.top() + 100.0), vec2(170.0, 30.0));
        if theme::button(ui, br, "open-ext", "Open a Media File...", Some(Icon::Open), Kind::Outline, true, 12.5).clicked() {
            app.open_external_file(&ctx);
        }
        return;
    }
    let mut clicked = None;
    theme::region(ui, list_r, "file-list", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for e in &app.library.entries {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
                let sel = app.selected.as_ref() == Some(&e.path);
                let pt = ui.painter();
                if sel {
                    pt.rect_filled(rect, 5.0, Color32::from_rgb(33, 62, 106));
                } else if resp.hovered() {
                    pt.rect_filled(rect, 5.0, FIELD_HOVER);
                }
                let th = Rect::from_min_size(pos2(rect.left() + 6.0, rect.top() + 7.0), vec2(64.0, 44.0));
                let is_video = e.info.as_ref().map(|i| i.has_video).unwrap_or_else(|| crate::library::is_video_ext(&e.path));
                if let Some(t) = &e.thumb {
                    pt.rect_filled(th, 3.0, Color32::BLACK);
                    let sz = t.size_vec2();
                    let fit = fit_rect(th, sz.x / sz.y.max(1.0));
                    pt.image(t.id(), fit, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                } else {
                    pt.rect_filled(th, 4.0, Color32::from_rgb(30, 44, 64));
                    icons::draw(pt, th.shrink(12.0), if is_video { Icon::Film } else { Icon::Wave }, BLUE_LIGHT);
                }
                let tx = th.right() + 10.0;
                let maxw = rect.right() - tx - 4.0;
                pt.text(pos2(tx, rect.top() + 6.0), Align2::LEFT_TOP, ellipsize(&e.title, maxw, 13.0), semi(13.0), TEXT);
                pt.text(pos2(tx, rect.top() + 23.0), Align2::LEFT_TOP, ellipsize(&e.subtitle, maxw, 11.5), reg(11.5), Color32::from_rgb(190, 202, 216));
                let dur = e.info.as_ref().map(|i| hms(i.duration)).unwrap_or_else(|| "--:--:--".into());
                pt.text(pos2(tx, rect.top() + 39.0), Align2::LEFT_TOP, format!("{dur}     {}", size_text(e.size)), reg(11.5), DIM);
                if resp.clicked() {
                    clicked = Some(e.path.clone());
                }
                let p = e.path.clone();
                resp.context_menu(|ui| {
                    if ui.button("Show in Explorer").clicked() {
                        let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", p.display())).spawn();
                        ui.close();
                    }
                });
            }
        });
    });
    if let Some(p) = clicked {
        app.select(p, &ctx);
    }
}

fn ellipsize(s: &str, max_w: f32, size: f32) -> String {
    let max_chars = (max_w / (size * 0.52)).max(4.0) as usize;
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    }
}

pub fn fit_rect(area: Rect, aspect: f32) -> Rect {
    let a = area.width() / area.height().max(1.0);
    if aspect > a {
        let h = area.width() / aspect;
        Rect::from_center_size(area.center(), vec2(area.width(), h))
    } else {
        let w = area.height() * aspect;
        Rect::from_center_size(area.center(), vec2(w, area.height()))
    }
}

// ---------------------------------------------------------------- preview
pub fn preview_area(app: &mut Studio, ui: &mut Ui, r: Rect, crop_tools: bool) {
    let pt = ui.painter();
    pt.rect_filled(r, 4.0, Color32::from_rgb(4, 6, 10));
    let info = app.info.clone();
    let Some(info) = info else {
        let msg = if app.selected.is_some() { "Loading..." } else { "Select a recording to preview" };
        pt.text(r.center(), Align2::CENTER_CENTER, msg, reg(13.0), DIM);
        return;
    };
    if !info.has_video {
        icons::draw(pt, Rect::from_center_size(r.center() - vec2(0.0, 14.0), vec2(56.0, 56.0)), Icon::Wave, BLUE_LIGHT);
        pt.text(r.center() + vec2(0.0, 32.0), Align2::CENTER_CENTER, "Audio file - see the waveform", reg(12.5), DIM);
        return;
    }
    let aspect = if info.height > 0 { info.width as f32 / info.height as f32 } else { 16.0 / 9.0 };
    let vr = fit_rect(r.shrink(2.0), aspect);
    if let Some(t) = &app.preview {
        pt.image(t.id(), vr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        pt.text(r.center(), Align2::CENTER_CENTER, "Loading preview...", reg(12.5), DIM);
    }
    // Crop overlay
    let (vw, vh) = (info.width as f32, info.height as f32);
    let to_screen = |x: f32, y: f32| pos2(vr.left() + vr.width() * x / vw, vr.top() + vr.height() * y / vh);
    let [cx, cy, cw, ch] = app.edit.crop;
    if cw > 0 && ch > 0 {
        let cr = Rect::from_two_pos(to_screen(cx as f32, cy as f32), to_screen((cx + cw) as f32, (cy + ch) as f32));
        let dim = Color32::from_black_alpha(140);
        pt.rect_filled(Rect::from_min_max(vr.min, pos2(vr.right(), cr.top())), 0.0, dim);
        pt.rect_filled(Rect::from_min_max(pos2(vr.left(), cr.bottom()), vr.max), 0.0, dim);
        pt.rect_filled(Rect::from_min_max(pos2(vr.left(), cr.top()), pos2(cr.left(), cr.bottom())), 0.0, dim);
        pt.rect_filled(Rect::from_min_max(pos2(cr.right(), cr.top()), pos2(vr.right(), cr.bottom())), 0.0, dim);
        pt.rect_stroke(cr, 0.0, Stroke::new(2.0, Color32::YELLOW), StrokeKind::Outside);
    }
    if app.crop_mode || crop_tools {
        let resp = ui.interact(vr, ui.id().with("crop-drag"), Sense::drag());
        if app.crop_mode {
            ui.painter().text(vr.left_top() + vec2(8.0, 8.0), Align2::LEFT_TOP, "Drag on the picture to choose the crop area", reg(12.0), Color32::YELLOW);
            let resp = resp.on_hover_cursor(egui::CursorIcon::Crosshair);
            if resp.drag_started() {
                app.crop_drag = resp.interact_pointer_pos();
            }
            if resp.dragged() {
                if let (Some(a), Some(b)) = (app.crop_drag, resp.interact_pointer_pos()) {
                    let fx = |p: f32| ((p - vr.left()) / vr.width()).clamp(0.0, 1.0) * vw;
                    let fy = |p: f32| ((p - vr.top()) / vr.height()).clamp(0.0, 1.0) * vh;
                    let x1 = fx(a.x.min(b.x)) as i32;
                    let x2 = fx(a.x.max(b.x)) as i32;
                    let y1 = fy(a.y.min(b.y)) as i32;
                    let y2 = fy(a.y.max(b.y)) as i32;
                    if x2 - x1 > 8 && y2 - y1 > 8 {
                        app.edit.crop = [x1 / 2 * 2, y1 / 2 * 2, (x2 - x1) / 2 * 2, (y2 - y1) / 2 * 2];
                    }
                }
            }
            if resp.drag_stopped() {
                app.crop_drag = None;
            }
        }
    }
}

pub fn transport(app: &mut Studio, ui: &mut Ui, r: Rect, ctx: &egui::Context) {
    let y = r.center().y;
    let has = app.info.is_some();
    let play_r = Rect::from_center_size(pos2(r.left() + 13.0, y), vec2(22.0, 22.0));
    let playing = app.playing();
    if theme::icon_plain(ui, play_r.shrink(2.0), "play", if playing { Icon::Pause } else { Icon::Play }, if playing { "Pause (Space)" } else { "Play (Space)" }).clicked() && has {
        app.toggle_play(ctx);
    }
    let dur = app.duration();
    let time = format!("{} / {}", clock(app.playhead), clock(dur));
    let tr = ui.painter().text(pos2(play_r.right() + 12.0, y), Align2::LEFT_CENTER, time, reg(12.5), TEXT);
    let fs_r = Rect::from_center_size(pos2(r.right() - 11.0, y), vec2(18.0, 18.0));
    let vol_w = (r.width() * 0.16).clamp(50.0, 90.0);
    let vol_r = Rect::from_min_max(pos2(fs_r.left() - 16.0 - vol_w, y - 8.0), pos2(fs_r.left() - 16.0, y + 8.0));
    let vi_r = Rect::from_center_size(pos2(vol_r.left() - 16.0, y), vec2(20.0, 20.0));
    let seek_r = Rect::from_min_max(pos2(tr.right() + 14.0, y - 9.0), pos2(vi_r.left() - 14.0, y + 9.0));
    if seek_r.width() > 20.0 {
        let v = if dur > 0.0 { (app.playhead / dur) as f32 } else { 0.0 };
        // trim range shading
        if dur > 0.0 && (app.edit.trim_start > 0.0 || app.edit.trim_end > 0.0) {
            let (s, e) = app.edit.selection(dur);
            let a = seek_r.left() + seek_r.width() * (s / dur) as f32;
            let b = seek_r.left() + seek_r.width() * (e / dur) as f32;
            ui.painter().rect_filled(Rect::from_min_max(pos2(a, y - 5.0), pos2(b, y + 5.0)), 2.0, Color32::from_rgba_unmultiplied(46, 204, 106, 50));
        }
        let (nv, released) = theme::slider_bar(ui, seek_r, "seek", v, BLUE);
        if let Some(nv) = nv {
            if dur > 0.0 {
                let t = nv as f64 * dur;
                if app.playing() {
                    // While playing, remember where the knob is and jump there on release.
                    app.seek_drag = Some(t);
                    app.playhead = t;
                    if released {
                        app.seek_drag = None;
                        app.seek(t, ctx);
                    }
                } else {
                    app.playhead = t;
                    app.request_still(t, ctx);
                }
            }
        } else if released {
            if let Some(t) = app.seek_drag.take() {
                app.seek(t, ctx);
            }
        }
    }
    let muted = app.muted || app.settings.preview_volume <= 0.001;
    if theme::icon_plain(ui, vi_r, "mute", if muted { Icon::Muted } else { Icon::Volume }, "Mute / unmute").clicked() {
        app.toggle_mute();
    }
    let (nv, _) = theme::slider_bar(ui, vol_r, "vol", if app.muted { 0.0 } else { app.settings.preview_volume }, BLUE);
    if let Some(v) = nv {
        app.set_volume(v);
    }
    if theme::icon_plain(ui, fs_r, "fullscreen", Icon::Fullscreen, "Full screen (Esc to exit)").clicked() && has {
        set_fullscreen(app, ctx, true);
    }
}

fn fullscreen(app: &mut Studio, ui: &mut Ui, full: Rect) {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(full, 0.0, Color32::BLACK);
    let bar = Rect::from_min_max(pos2(full.left() + 20.0, full.bottom() - 46.0), pos2(full.right() - 20.0, full.bottom() - 10.0));
    let video = Rect::from_min_max(full.min, pos2(full.right(), bar.top() - 8.0));
    preview_area(app, ui, video, false);
    ui.painter().rect_filled(bar.expand(4.0), 6.0, Color32::from_rgba_unmultiplied(20, 27, 38, 230));
    transport(app, ui, bar, &ctx);
    if ui.interact(video, ui.id().with("fs-dbl"), Sense::click()).double_clicked() {
        set_fullscreen(app, &ctx, false);
    }
}

// ---------------------------------------------------------------- waveform
pub fn waveform_view(app: &mut Studio, ui: &mut Ui, r: Rect, ctx: &egui::Context) {
    let pt = ui.painter();
    pt.rect_filled(r, 4.0, Color32::from_rgb(12, 18, 27));
    let axis_h = 18.0;
    let area = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - axis_h));
    let Some(w) = &app.waveform else {
        let msg = if app.selected.is_none() {
            "Select a recording to see its waveform"
        } else if app.wave_loading {
            "Reading audio..."
        } else {
            "This file has no audio track"
        };
        pt.text(area.center(), Align2::CENTER_CENTER, msg, reg(12.5), DIM);
        return;
    };
    let dur = app.duration().max(w.duration).max(0.001);
    let lanes = [(&w.left, "Left Channel", WAVE_L), (&w.right, "Right Channel", WAVE_R)];
    let lane_h = area.height() / 2.0;
    for (k, (data, name, color)) in lanes.iter().enumerate() {
        let lr = Rect::from_min_size(pos2(area.left(), area.top() + k as f32 * lane_h), vec2(area.width(), lane_h));
        let mid = lr.center().y + 6.0;
        let amp = (lr.height() - 26.0) / 2.0;
        pt.text(pos2(lr.left() + 8.0, lr.top() + 4.0), Align2::LEFT_TOP, *name, reg(11.5), TEXT);
        let n = data.len().max(1);
        let cols = lr.width() as usize;
        let span = w.duration / dur;
        for c in 0..cols {
            let f0 = c as f64 / cols as f64 / span;
            let f1 = (c + 1) as f64 / cols as f64 / span;
            if f0 >= 1.0 {
                break;
            }
            let i0 = (f0 * n as f64) as usize;
            let i1 = ((f1 * n as f64) as usize).clamp(i0 + 1, n);
            let peak = data[i0.min(n - 1)..i1].iter().cloned().fold(0.0f32, f32::max).min(1.0);
            let h = (peak.sqrt() * amp).max(0.5);
            let x = lr.left() + c as f32 + 0.5;
            pt.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, *color));
        }
    }
    // Trim selection
    let tx = |t: f64| area.left() + area.width() * (t / dur) as f32;
    if app.edit.trim_start > 0.0 || app.edit.trim_end > 0.0 {
        let (s, e) = app.edit.selection(dur);
        let shade = Color32::from_black_alpha(150);
        pt.rect_filled(Rect::from_min_max(area.min, pos2(tx(s), area.bottom())), 0.0, shade);
        pt.rect_filled(Rect::from_min_max(pos2(tx(e), area.top()), area.max), 0.0, shade);
        pt.line_segment([pos2(tx(s), area.top()), pos2(tx(s), area.bottom())], Stroke::new(2.0, GREEN));
        pt.line_segment([pos2(tx(e), area.top()), pos2(tx(e), area.bottom())], Stroke::new(2.0, ORANGE));
    }
    // Playhead
    let px = tx(app.playhead);
    pt.line_segment([pos2(px, area.top()), pos2(px, area.bottom())], Stroke::new(1.5, Color32::WHITE));
    // Time axis
    let steps = [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];
    let step = steps.iter().cloned().find(|s| dur / s <= 7.0).unwrap_or(7200.0);
    let mut t = 0.0;
    while t <= dur + 0.001 {
        let x = tx(t);
        let anchor = if t == 0.0 { Align2::LEFT_TOP } else if x > area.right() - 20.0 { Align2::RIGHT_TOP } else { Align2::CENTER_TOP };
        pt.text(pos2(x, area.bottom() + 3.0), anchor, clock(t), reg(10.5), DIM);
        t += step;
    }
    // Interaction: click = move playhead, drag = choose the trim range, double-click = clear trim.
    let resp = ui.interact(area, ui.id().with("wave"), Sense::click_and_drag());
    if resp.hovered() {
        ui.painter().text(pos2(area.right() - 8.0, area.top() + 4.0), Align2::RIGHT_TOP, "Click = move playhead   Drag = select trim   Double-click = clear trim", reg(11.0), DIM);
    }
    let to_t = |x: f32| ((x - area.left()) / area.width()).clamp(0.0, 1.0) as f64 * dur;
    if resp.double_clicked() {
        app.edit.trim_start = 0.0;
        app.edit.trim_end = 0.0;
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            app.seek(to_t(p.x), ctx);
        }
    }
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            let id = ui.id().with("wave-anchor");
            ui.data_mut(|d| d.insert_temp(id, to_t(p.x)));
        }
    }
    if resp.dragged() {
        let id = ui.id().with("wave-anchor");
        if let (Some(a), Some(p)) = (ui.data(|d| d.get_temp::<f64>(id)), resp.interact_pointer_pos()) {
            let b = to_t(p.x);
            app.edit.trim_start = a.min(b);
            app.edit.trim_end = a.max(b);
        }
    }
}

// ---------------------------------------------------------------- status bar
fn status_bar(app: &mut Studio, ui: &mut Ui, r: Rect) {
    let pt = ui.painter();
    pt.rect_filled(r, 0.0, HEADER);
    pt.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, PANEL_BORDER));
    let col = match app.status_kind {
        StatusKind::Ok => GREEN,
        StatusKind::Busy => ORANGE,
        StatusKind::Warn => ORANGE,
        StatusKind::Error => RED,
    };
    let col = if app.recording() { RED } else { col };
    pt.circle_filled(pos2(r.left() + 18.0, r.center().y), 5.5, col);
    // Right side: output folder
    let open_r = Rect::from_min_size(pos2(r.right() - 104.0, r.top() + 3.0), vec2(96.0, r.height() - 6.0));
    let folder = app.settings.output_dir.display().to_string();
    let max_chars = ((r.width() * 0.36) / (12.0 * 0.52)) as usize;
    let folder = if folder.chars().count() > max_chars {
        let tail: String = folder.chars().rev().take(max_chars.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        folder
    };
    let ftxt = format!("Output Folder:   {folder}");
    let fr = pt.text(pos2(open_r.left() - 12.0, r.center().y), Align2::RIGHT_CENTER, ftxt, reg(12.0), DIM);
    if theme::button(ui, open_r, "status-open", "Open Folder", Some(Icon::Open), Kind::Outline, true, 12.0).clicked() {
        app.open_folder(&app.settings.output_dir.clone());
    }
    // Job progress
    let mut text_right = fr.left() - 16.0;
    if let Some(job) = &app.job {
        let p = job.progress();
        let cancel_r = Rect::from_min_size(pos2(text_right - 64.0, r.top() + 4.0), vec2(60.0, r.height() - 8.0));
        let can_cancel = job.cancellable;
        let bar = Rect::from_min_size(pos2(cancel_r.left() - 196.0, r.center().y - 5.0), vec2(160.0, 10.0));
        let pt = ui.painter();
        pt.rect_filled(bar, 5.0, Color32::from_rgb(40, 50, 66));
        pt.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * p, bar.height())), 5.0, BLUE);
        pt.text(pos2(bar.right() + 6.0, r.center().y), Align2::LEFT_CENTER, format!("{:.0}%", p * 100.0), reg(11.5), TEXT);
        let detail = job.detail();
        if !detail.is_empty() {
            pt.text(pos2(bar.left() - 8.0, r.center().y), Align2::RIGHT_CENTER, ellipsize(&detail, 220.0, 11.5), reg(11.5), DIM);
        }
        let jc = can_cancel;
        if theme::button(ui, cancel_r, "job-cancel", "Cancel", None, Kind::Outline, jc, 11.5).clicked() {
            if let Some(j) = &app.job {
                j.cancel();
            }
        }
        text_right = bar.left() - 240.0;
    }
    let max_w = (text_right - r.left() - 34.0).max(40.0);
    let status = if app.recording() {
        let s = app.record_seconds();
        format!("Recording  {:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        app.status.clone()
    };
    let pt = ui.painter();
    let shown = ellipsize(&status, max_w, 12.5);
    let sr = pt.text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, shown, reg(12.5), TEXT);
    if status.chars().count() > (max_w / (12.5 * 0.52)) as usize {
        ui.interact(sr, ui.id().with("status-tip"), Sense::hover()).on_hover_text(status);
    }
}
