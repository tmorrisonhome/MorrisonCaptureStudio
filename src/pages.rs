//! Editor tabs, the other top-menu pages (Edit/Timeline, Media Library, Export, Settings) and dialogs.
use crate::app::{Dialog, Page, Studio, Tab};
use crate::edit::{self, clock, EditState, FORMATS, QUALITY};
use crate::icons::{self, Icon};
use crate::library::{hms, size_text};
use crate::theme::{self, *};
use crate::ui::{columns, file_list, preview_area, transport, waveform_view};
#[cfg(not(windows))]
use crate::preview_stubs::rfd;
use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui};

fn heading(ui: &mut Ui, t: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(t).font(semi(14.0)).color(BLUE_LIGHT));
    ui.add_space(2.0);
}

fn small_buttons(app: &mut Studio, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.add_enabled(app.can_undo(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() {
            app.undo();
        }
        if ui.add_enabled(app.can_redo(), egui::Button::new("Redo")).on_hover_text("Ctrl+Y").clicked() {
            app.redo();
        }
        if ui.button("Reset All Edits").clicked() {
            app.reset_edits();
        }
        if ui.button("Go to Export...").clicked() {
            app.page = Page::Export;
        }
    });
}

// ---------------------------------------------------------------- audio editor tab
pub fn audio_editor_tab(app: &mut Studio, ui: &mut Ui, content: Rect) {
    let ctx = ui.ctx().clone();
    let (left, mid, right) = columns(content);
    file_list(app, ui, left);
    let work = Rect::from_min_max(mid.min, right.max);
    let inner = theme::panel(ui, work, "Audio Editor");
    let wave_h = (inner.height() * 0.42).max(140.0);
    let wave = Rect::from_min_size(inner.min, vec2(inner.width(), wave_h));
    waveform_view(app, ui, wave, &ctx);
    let tr = Rect::from_min_size(pos2(inner.left(), wave.bottom() + 6.0), vec2(inner.width(), 30.0));
    transport(app, ui, tr, &ctx);
    let controls = Rect::from_min_max(pos2(inner.left(), tr.bottom() + 8.0), inner.max);
    let has = app.info.as_ref().map(|i| i.has_audio).unwrap_or(false);
    theme::region(ui, controls, "audio-controls", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_enabled_ui(has, |ui| {
                ui.columns(2, |c| {
                    let dur = app.duration();
                    heading(&mut c[0], "Trim");
                    c[0].horizontal(|ui| {
                        ui.label("Start");
                        ui.add(egui::DragValue::new(&mut app.edit.trim_start).range(0.0..=dur.max(0.0)).speed(0.05).custom_formatter(|v, _| format!("{v:.2} s")));
                        ui.label("End");
                        ui.add(egui::DragValue::new(&mut app.edit.trim_end).range(0.0..=dur.max(0.0)).speed(0.05).custom_formatter(|v, _| if v <= 0.0 { "end".into() } else { format!("{v:.2} s") }));
                    });
                    c[0].horizontal(|ui| {
                        if ui.button("Start = Playhead").clicked() {
                            app.edit.trim_start = app.playhead;
                        }
                        if ui.button("End = Playhead").clicked() {
                            app.edit.trim_end = app.playhead;
                        }
                        if ui.button("Clear").clicked() {
                            app.edit.trim_start = 0.0;
                            app.edit.trim_end = 0.0;
                        }
                    });
                    heading(&mut c[0], "Level and speed");
                    egui::Grid::new("lvl").num_columns(2).spacing([10.0, 8.0]).show(&mut c[0], |ui| {
                        ui.label("Volume (dB)");
                        ui.add(egui::Slider::new(&mut app.edit.gain_db, -30.0..=24.0).step_by(0.5));
                        ui.end_row();
                        ui.label("Speed");
                        ui.add(egui::Slider::new(&mut app.edit.speed, 0.25..=4.0).step_by(0.05).suffix("x"));
                        ui.end_row();
                        ui.label("Fade in (s)");
                        ui.add(egui::Slider::new(&mut app.edit.fade_in, 0.0..=10.0).step_by(0.1));
                        ui.end_row();
                        ui.label("Fade out (s)");
                        ui.add(egui::Slider::new(&mut app.edit.fade_out, 0.0..=10.0).step_by(0.1));
                        ui.end_row();
                    });
                    heading(&mut c[1], "Tone");
                    egui::Grid::new("tone").num_columns(2).spacing([10.0, 8.0]).show(&mut c[1], |ui| {
                        ui.label("Bass (dB)");
                        ui.add(egui::Slider::new(&mut app.edit.bass_db, -15.0..=15.0).step_by(0.5));
                        ui.end_row();
                        ui.label("Treble (dB)");
                        ui.add(egui::Slider::new(&mut app.edit.treble_db, -15.0..=15.0).step_by(0.5));
                        ui.end_row();
                    });
                    heading(&mut c[1], "Clean-up and effects");
                    c[1].checkbox(&mut app.edit.normalize, "Normalize loudness (even, broadcast-style level)");
                    c[1].checkbox(&mut app.edit.noise_reduce, "Reduce background noise");
                    c[1].checkbox(&mut app.edit.compressor, "Compressor (evens out loud and quiet parts)");
                    c[1].checkbox(&mut app.edit.mono, "Convert to mono");
                    c[1].checkbox(&mut app.edit.reverse_audio, "Reverse audio");
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Edits are applied when you export (Export Selected). The preview plays the original file.").small().color(DIM));
                small_buttons(app, ui);
            });
            if !has {
                ui.label(RichText::new("Select a file that has sound to use the audio editor.").color(DIM));
            }
        });
    });
}

// ---------------------------------------------------------------- video editor tab
pub fn video_editor_tab(app: &mut Studio, ui: &mut Ui, content: Rect) {
    let ctx = ui.ctx().clone();
    let (left, mid, right) = columns(content);
    file_list(app, ui, left);
    let pv = theme::panel(ui, mid, "Video Preview");
    let vr = Rect::from_min_max(pv.min, pos2(pv.right(), pv.bottom() - 36.0));
    preview_area(app, ui, vr, true);
    transport(app, ui, Rect::from_min_max(pos2(pv.left(), vr.bottom() + 4.0), pv.max), &ctx);
    let inner = theme::panel(ui, right, "Video Editor");
    let info = app.info.clone().unwrap_or_default();
    let has = info.has_video;
    theme::region(ui, inner, "video-controls", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !has {
                ui.label(RichText::new("Select a video file to use the video editor.").color(DIM));
            }
            ui.add_enabled_ui(has, |ui| {
                heading(ui, "Crop");
                ui.horizontal(|ui| {
                    let txt = if app.crop_mode { "Drawing: drag on the preview (click here to stop)" } else { "Draw Crop on Preview" };
                    if ui.add(egui::Button::new(txt).selected(app.crop_mode)).clicked() {
                        app.crop_mode = !app.crop_mode;
                    }
                    if ui.button("Clear crop").clicked() {
                        app.edit.crop = [0; 4];
                    }
                });
                ui.horizontal(|ui| {
                    let (w, h) = (info.width as i32, info.height as i32);
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut app.edit.crop[0]).range(0..=w).speed(2));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut app.edit.crop[1]).range(0..=h).speed(2));
                    ui.label("W");
                    ui.add(egui::DragValue::new(&mut app.edit.crop[2]).range(0..=w).speed(2));
                    ui.label("H");
                    ui.add(egui::DragValue::new(&mut app.edit.crop[3]).range(0..=h).speed(2));
                });
                ui.label(RichText::new(format!("Source size: {} x {}", info.width, info.height)).small().color(DIM));
                heading(ui, "Rotate and flip");
                ui.horizontal(|ui| {
                    for d in [0, 90, 180, 270] {
                        ui.selectable_value(&mut app.edit.rotation, d, format!("{d}°"));
                    }
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.edit.flip_h, "Flip horizontal");
                    ui.checkbox(&mut app.edit.flip_v, "Flip vertical");
                });
                heading(ui, "Colour");
                egui::Grid::new("color").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Brightness");
                    ui.add(egui::Slider::new(&mut app.edit.brightness, -0.5..=0.5).step_by(0.01));
                    ui.end_row();
                    ui.label("Contrast");
                    ui.add(egui::Slider::new(&mut app.edit.contrast, 0.5..=2.0).step_by(0.01));
                    ui.end_row();
                    ui.label("Saturation");
                    ui.add(egui::Slider::new(&mut app.edit.saturation, 0.0..=3.0).step_by(0.01));
                    ui.end_row();
                });
                ui.checkbox(&mut app.edit.grayscale, "Black and white");
                heading(ui, "Size and sound");
                ui.horizontal(|ui| {
                    ui.label("Resize to");
                    ui.add(egui::DragValue::new(&mut app.edit.resize[0]).range(0..=7680).speed(2));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut app.edit.resize[1]).range(0..=4320).speed(2));
                    ui.label(RichText::new("(0 = keep)").small().color(DIM));
                });
                ui.horizontal(|ui| {
                    for (l, w, h) in [("1080p", 1920, 1080), ("720p", 1280, 720), ("480p", 854, 480)] {
                        if ui.small_button(l).clicked() {
                            app.edit.resize = [w, h];
                        }
                    }
                    if ui.small_button("Original").clicked() {
                        app.edit.resize = [0, 0];
                    }
                });
                ui.checkbox(&mut app.edit.mute_audio, "Remove the sound from the exported video");
                ui.checkbox(&mut app.edit.video_fades, "Fade the picture too when audio fades are set");
                ui.add_space(6.0);
                small_buttons(app, ui);
            });
        });
    });
}

// ---------------------------------------------------------------- edit / timeline page
pub fn timeline_page(app: &mut Studio, ui: &mut Ui, body: Rect) {
    let ctx = ui.ctx().clone();
    let gap = 10.0;
    let top_h = (body.height() - gap) * 0.5;
    let vr = Rect::from_min_size(body.min, vec2(body.width(), top_h));
    let ar = Rect::from_min_max(pos2(body.left(), vr.bottom() + gap), body.max);

    // ---- video clip timeline
    let inner = theme::panel(ui, vr, "Video Clip Timeline");
    let bw = 150.0;
    let mut bx = vr.right() - 12.0;
    let mut place = |w: f32| {
        bx -= w;
        let r = Rect::from_min_size(pos2(bx, vr.top() + 7.0), vec2(w, 28.0));
        bx -= 8.0;
        r
    };
    let r_render = place(bw + 30.0);
    let r_add = place(bw);
    let r_save = place(120.0);
    let r_open = place(120.0);
    if theme::button(ui, r_render, "tl-render", "Render Timeline (MP4)", Some(Icon::Film), Kind::Primary, !app.clips.is_empty() && !app.busy(), 12.5).clicked() {
        app.render_timeline(&ctx);
    }
    if theme::button(ui, r_add, "tl-add", "Add Selected File", Some(Icon::Plus), Kind::Normal, app.selected.is_some(), 12.5).clicked() {
        app.add_clip();
    }
    if theme::button(ui, r_save, "proj-save", "Save Project", Some(Icon::Save), Kind::Outline, true, 12.0).clicked() {
        app.save_project();
    }
    if theme::button(ui, r_open, "proj-open", "Open Project", Some(Icon::Open), Kind::Outline, true, 12.0).clicked() {
        app.open_project(&ctx);
    }
    let strip = Rect::from_min_size(inner.min, vec2(inner.width(), 70.0));
    ui.painter().rect_filled(strip, 4.0, Color32::from_rgb(12, 18, 27));
    let total: f64 = app.clips.iter().map(|c| (c.end - c.start).max(0.0)).sum();
    if app.clips.is_empty() {
        ui.painter().text(strip.center(), Align2::CENTER_CENTER, "Select a video in the Recordings list, set its trim In/Out if you like, then press Add Selected File.", reg(12.5), DIM);
    } else {
        let mut x = strip.left() + 4.0;
        let usable = strip.width() - 8.0 - 4.0 * app.clips.len() as f32;
        let mut clicked = None;
        for (i, c) in app.clips.iter().enumerate() {
            let len = (c.end - c.start).max(0.0);
            let w = ((len / total.max(0.001)) as f32 * usable).max(56.0);
            let cr = Rect::from_min_size(pos2(x, strip.top() + 6.0), vec2(w.min(strip.right() - 4.0 - x).max(10.0), strip.height() - 12.0));
            x += w + 4.0;
            let resp = ui.interact(cr, ui.id().with(("clip", i)), Sense::click());
            let sel = app.clip_sel == Some(i);
            let pt = ui.painter();
            pt.rect_filled(cr, 5.0, if sel { Color32::from_rgb(40, 84, 150) } else { Color32::from_rgb(34, 58, 92) });
            pt.rect_stroke(cr, 5.0, Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { BLUE_LIGHT } else { FIELD_BORDER }), StrokeKind::Inside);
            let name = std::path::Path::new(&c.path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let clipped = pt.with_clip_rect(cr.shrink(4.0));
            clipped.text(cr.left_top() + vec2(8.0, 8.0), Align2::LEFT_TOP, format!("{}. {}", i + 1, name), semi(12.0), TEXT);
            clipped.text(cr.left_top() + vec2(8.0, 28.0), Align2::LEFT_TOP, format!("{} - {}  ({})", clock(c.start), clock(c.end), clock(len)), reg(11.0), DIM);
            if resp.clicked() {
                clicked = Some(i);
            }
        }
        if let Some(i) = clicked {
            app.clip_sel = Some(i);
        }
    }
    let details = Rect::from_min_max(pos2(inner.left(), strip.bottom() + 10.0), inner.max);
    theme::region(ui, details, "clip-details", |ui| {
        ui.label(RichText::new(format!("{} clip(s), total length {}. Output: 1280 x 720, 30 fps MP4 saved to the output folder.", app.clips.len(), clock(total))).color(DIM));
        if let Some(i) = app.clip_sel.filter(|i| *i < app.clips.len()) {
            ui.add_space(4.0);
            let n = app.clips.len();
            let mut remove = false;
            let mut mv: i32 = 0;
            let playhead = app.playhead;
            let c = &mut app.clips[i];
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Clip {}", i + 1)).font(semi(13.5)));
                ui.label("In");
                ui.add(egui::DragValue::new(&mut c.start).range(0.0..=c.end).speed(0.05).custom_formatter(|v, _| format!("{v:.2} s")));
                ui.label("Out");
                ui.add(egui::DragValue::new(&mut c.end).range(c.start..=1.0e6).speed(0.05).custom_formatter(|v, _| format!("{v:.2} s")));
                if ui.button("In = Playhead").on_hover_text("Uses the playhead position of the selected file").clicked() {
                    c.start = playhead.min(c.end);
                }
                if ui.button("Out = Playhead").clicked() {
                    c.end = playhead.max(c.start);
                }
                if ui.add_enabled(i > 0, egui::Button::new("Move Left")).clicked() {
                    mv = -1;
                }
                if ui.add_enabled(i + 1 < n, egui::Button::new("Move Right")).clicked() {
                    mv = 1;
                }
                if ui.button("Remove").clicked() {
                    remove = true;
                }
            });
            ui.label(RichText::new(&app.clips[i].path).small().color(DIM));
            if remove {
                app.clips.remove(i);
                app.clip_sel = None;
            } else if mv != 0 {
                let j = (i as i32 + mv) as usize;
                app.clips.swap(i, j);
                app.clip_sel = Some(j);
            }
        }
    });

    // ---- multitrack audio
    let inner = theme::panel(ui, ar, "Multitrack Audio Mix");
    let mut bx = ar.right() - 12.0;
    let mut place = |w: f32| {
        bx -= w;
        let r = Rect::from_min_size(pos2(bx, ar.top() + 7.0), vec2(w, 28.0));
        bx -= 8.0;
        r
    };
    let r_mix = place(130.0);
    let r_fmt = place(100.0);
    let r_add = place(bw);
    if theme::button(ui, r_mix, "mix-export", "Export Mix", Some(Icon::Export), Kind::Primary, app.tracks.iter().any(|t| !t.muted) && !app.busy(), 12.5).clicked() {
        app.export_mix(&ctx);
    }
    let fmts = ["WAV", "MP3", "FLAC", "M4A"];
    let mut mf = app.mix_format;
    theme::combo(ui, r_fmt.shrink2(vec2(0.0, 1.0)), "mix-fmt", fmts[mf.min(3)], |ui| {
        for (i, f) in fmts.iter().enumerate() {
            ui.selectable_value(&mut mf, i, *f);
        }
    });
    app.mix_format = mf;
    if theme::button(ui, r_add, "mix-add", "Add Selected File", Some(Icon::Plus), Kind::Normal, app.selected.is_some(), 12.5).clicked() {
        app.add_track();
    }
    let span = app.tracks.iter().map(|t| t.start + t.duration).fold(10.0f64, f64::max) * 1.1;
    if app.tracks.is_empty() {
        ui.painter().text(pos2(inner.center().x, inner.top() + 40.0), Align2::CENTER_CENTER, "Add recordings as tracks, slide them in time, set each level, then Export Mix.", reg(12.5), DIM);
    }
    let lanes_r = inner;
    let head_w = 300.0;
    let mut remove = None;
    theme::region(ui, lanes_r, "lanes", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (i, t) in app.tracks.iter_mut().enumerate() {
                let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
                let pt = ui.painter();
                pt.rect_filled(row, 4.0, Color32::from_rgb(24, 33, 46));
                let head = Rect::from_min_size(row.min, vec2(head_w, row.height()));
                let lane = Rect::from_min_max(pos2(head.right() + 8.0, row.top() + 5.0), pos2(row.right() - 6.0, row.bottom() - 5.0));
                pt.rect_filled(lane, 3.0, Color32::from_rgb(12, 18, 27));
                let name = std::path::Path::new(&t.path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                theme::region(ui, head.shrink2(vec2(8.0, 3.0)), &format!("head{i}"), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{}. {}", i + 1, name)).font(semi(12.0)));
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut t.muted, "Mute");
                        ui.add(egui::DragValue::new(&mut t.gain_db).range(-60.0..=24.0).speed(0.2).suffix(" dB"));
                        ui.add(egui::DragValue::new(&mut t.start).range(0.0..=36000.0).speed(0.05).prefix("at ").suffix(" s"));
                        if ui.small_button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
                let x0 = lane.left() + lane.width() * (t.start / span) as f32;
                let x1 = lane.left() + lane.width() * ((t.start + t.duration.max(0.5)) / span) as f32;
                let block = Rect::from_min_max(pos2(x0, lane.top() + 2.0), pos2(x1.min(lane.right()), lane.bottom() - 2.0));
                let resp = ui.interact(block, ui.id().with(("blk", i)), Sense::drag()).on_hover_cursor(egui::CursorIcon::Grab);
                if resp.dragged() {
                    t.start = (t.start + (resp.drag_delta().x / lane.width()) as f64 * span).max(0.0);
                }
                let col = if t.muted { Color32::from_rgb(70, 76, 88) } else { WAVE_L };
                let pt = ui.painter();
                pt.rect_filled(block, 3.0, col.gamma_multiply(0.75));
                pt.with_clip_rect(block).text(block.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, clock(t.duration), reg(11.0), Color32::WHITE);
            }
        });
    });
    if let Some(i) = remove {
        app.tracks.remove(i);
    }
}

// ---------------------------------------------------------------- media library page
pub fn library_page(app: &mut Studio, ui: &mut Ui, body: Rect) {
    let ctx = ui.ctx().clone();
    let inner = theme::panel(ui, body, &format!("Media Library ({} files)", app.library.entries.len()));
    let bar = Rect::from_min_size(inner.min, vec2(inner.width(), 30.0));
    let mut open_sel = None;
    let mut action: Option<(u8, std::path::PathBuf)> = None;
    theme::region(ui, bar, "lib-bar", |ui| {
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add(egui::TextEdit::singleline(&mut app.search).desired_width(220.0).hint_text("file name"));
            let filters = ["All files", "Video only", "Audio only"];
            egui::ComboBox::from_id_salt("lib-filter").selected_text(filters[app.lib_filter]).show_ui(ui, |ui| {
                for (i, f) in filters.iter().enumerate() {
                    ui.selectable_value(&mut app.lib_filter, i, *f);
                }
            });
            if ui.button("Refresh").clicked() {
                app.library.refresh(&app.settings.output_dir.clone());
            }
            if ui.button("Add Files to Library...").clicked() {
                action = Some((9, Default::default()));
            }
            if ui.button("Open Other File...").clicked() {
                action = Some((8, Default::default()));
            }
            if ui.button("Open Output Folder").clicked() {
                app.open_folder(&app.settings.output_dir.clone());
            }
        });
    });
    let list = Rect::from_min_max(pos2(inner.left(), bar.bottom() + 8.0), inner.max);
    let q = app.search.to_lowercase();
    theme::region(ui, list, "lib-list", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for e in &app.library.entries {
                let is_video = e.info.as_ref().map(|i| i.has_video).unwrap_or_else(|| crate::library::is_video_ext(&e.path));
                if (app.lib_filter == 1 && !is_video) || (app.lib_filter == 2 && is_video) {
                    continue;
                }
                let name = edit::name(&e.path);
                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                    continue;
                }
                let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 66.0), Sense::click());
                let sel = app.selected.as_ref() == Some(&e.path);
                let pt = ui.painter();
                pt.rect_filled(row, 5.0, if sel { Color32::from_rgb(33, 62, 106) } else if resp.hovered() { FIELD_HOVER } else { Color32::from_rgb(23, 32, 45) });
                let th = Rect::from_min_size(pos2(row.left() + 8.0, row.top() + 6.0), vec2(96.0, 54.0));
                pt.rect_filled(th, 3.0, Color32::BLACK);
                if let Some(t) = &e.thumb {
                    let sz = t.size_vec2();
                    pt.image(t.id(), crate::ui::fit_rect(th, sz.x / sz.y.max(1.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                } else {
                    icons::draw(pt, th.shrink(14.0), if is_video { Icon::Film } else { Icon::Wave }, BLUE_LIGHT);
                }
                let x = th.right() + 14.0;
                pt.text(pos2(x, row.top() + 10.0), Align2::LEFT_TOP, &name, semi(13.5), TEXT);
                let info = e.info.as_ref();
                let kind = match info {
                    Some(i) if i.has_video => format!("Video  {} x {}{}", i.width, i.height, if i.has_audio { "  with sound" } else { "  no sound" }),
                    Some(_) => "Audio".to_string(),
                    None => "Reading...".to_string(),
                };
                pt.text(pos2(x, row.top() + 32.0), Align2::LEFT_TOP, format!("{kind}     Length {}     {}", info.map(|i| hms(i.duration)).unwrap_or_default(), size_text(e.size)), reg(12.0), DIM);
                // action buttons
                let mut bx = row.right() - 8.0;
                for (code, txt, icon) in [(4u8, "Delete", Icon::Trash), (3, "Show in Explorer", Icon::Folder), (2, "Rename", Icon::Pencil), (1, "Open", Icon::Play)] {
                    let w = if code == 3 { 150.0 } else { 92.0 };
                    bx -= w;
                    let br = Rect::from_min_size(pos2(bx, row.center().y - 14.0), vec2(w, 28.0));
                    bx -= 6.0;
                    if theme::button(ui, br, &format!("lib{code}{name}"), txt, Some(icon), if code == 1 { Kind::Primary } else { Kind::Outline }, true, 12.0).clicked() {
                        action = Some((code, e.path.clone()));
                    }
                }
                if resp.double_clicked() {
                    open_sel = Some(e.path.clone());
                } else if resp.clicked() {
                    action = Some((0, e.path.clone()));
                }
            }
        });
    });
    if let Some(p) = open_sel {
        app.select(p, &ctx);
        app.page = Page::Record;
        app.tab = Tab::Recordings;
    }
    if let Some((code, p)) = action {
        match code {
            0 => app.select(p, &ctx),
            1 => {
                app.select(p, &ctx);
                app.page = Page::Record;
                app.tab = Tab::Recordings;
            }
            2 => {
                let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
                app.dialog = Dialog::Rename(p, stem);
            }
            3 => app.show_in_explorer(&p),
            4 => app.dialog = Dialog::Delete(p),
            8 => app.open_external_file(&ctx),
            9 => app.import_files(&ctx),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------- export page
pub fn export_page(app: &mut Studio, ui: &mut Ui, body: Rect) {
    let ctx = ui.ctx().clone();
    let gap = 10.0;
    let lw = (body.width() * 0.55).max(520.0).min(body.width() - 300.0);
    let lr = Rect::from_min_size(body.min, vec2(lw, body.height()));
    let rr = Rect::from_min_max(pos2(lr.right() + gap, body.top()), body.max);
    let inner = theme::panel(ui, lr, "Export Settings");
    theme::region(ui, inner, "export-settings", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            heading(ui, "Source file");
            match (&app.selected, &app.info) {
                (Some(p), Some(i)) => {
                    ui.label(RichText::new(edit::name(p)).font(semi(13.5)));
                    let kind = if i.has_video { format!("Video {} x {}, {}", i.width, i.height, if i.has_audio { "with sound" } else { "no sound" }) } else { "Audio".into() };
                    ui.label(RichText::new(format!("{kind}, length {}", hms(i.duration))).color(DIM));
                }
                _ => {
                    ui.label(RichText::new("No file selected. Pick one in Recordings or the Media Library.").color(ORANGE));
                }
            }
            heading(ui, "Format and quality");
            egui::Grid::new("fmtq").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Format");
                let mut f = app.settings.export_format.min(FORMATS.len() - 1);
                egui::ComboBox::from_id_salt("pg-fmt").width(240.0).selected_text(FORMATS[f].label).show_ui(ui, |ui| {
                    for (i, fm) in FORMATS.iter().enumerate() {
                        ui.selectable_value(&mut f, i, fm.label);
                    }
                });
                ui.end_row();
                ui.label("Quality");
                let mut q = app.settings.export_quality.min(QUALITY.len() - 1);
                egui::ComboBox::from_id_salt("pg-q").width(240.0).selected_text(QUALITY[q]).show_ui(ui, |ui| {
                    for (i, l) in QUALITY.iter().enumerate() {
                        ui.selectable_value(&mut q, i, *l);
                    }
                });
                ui.end_row();
                if f != app.settings.export_format || q != app.settings.export_quality {
                    app.settings.export_format = f;
                    app.settings.export_quality = q;
                    app.settings.save();
                }
            });
            heading(ui, "Save as");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.export_path).desired_width(ui.available_width() - 190.0).hint_text("Automatic: \"<name> (Edited).<format>\" in the output folder"));
                if ui.button("Browse...").clicked() {
                    let fmt = &FORMATS[app.settings.export_format.min(FORMATS.len() - 1)];
                    if let Some(p) = rfd::FileDialog::new().add_filter(fmt.label, &[fmt.ext]).set_directory(&app.settings.output_dir).save_file() {
                        app.export_path = p.with_extension(fmt.ext).to_string_lossy().to_string();
                    }
                }
                if ui.button("Automatic").clicked() {
                    app.export_path.clear();
                }
            });
            ui.label(RichText::new("The file extension always follows the chosen format.").small().color(DIM));
            heading(ui, "Trim");
            let dur = app.duration();
            ui.horizontal(|ui| {
                ui.label("Start");
                ui.add(egui::DragValue::new(&mut app.edit.trim_start).range(0.0..=dur.max(0.0)).speed(0.05).custom_formatter(|v, _| format!("{v:.2} s")));
                ui.label("End");
                ui.add(egui::DragValue::new(&mut app.edit.trim_end).range(0.0..=dur.max(0.0)).speed(0.05).custom_formatter(|v, _| if v <= 0.0 { "end".into() } else { format!("{v:.2} s") }));
                if ui.button("Clear").clicked() {
                    app.edit.trim_start = 0.0;
                    app.edit.trim_end = 0.0;
                }
            });
            heading(ui, "Edits that will be applied");
            let sum = app.edit.summary();
            if sum.is_empty() {
                ui.label(RichText::new("None - the file is converted as it is.").color(DIM));
            } else {
                for s in sum {
                    ui.label(format!("•  {s}"));
                }
            }
            ui.horizontal(|ui| {
                if ui.button("Open Audio Editor").clicked() {
                    app.page = Page::Record;
                    app.tab = Tab::AudioEditor;
                }
                if ui.button("Open Video Editor").clicked() {
                    app.page = Page::Record;
                    app.tab = Tab::VideoEditor;
                }
                if ui.button("Reset All Edits").clicked() {
                    app.reset_edits();
                }
            });
            ui.add_space(12.0);
            let can = app.selected.is_some() && app.info.is_some() && !app.busy() && !app.recording();
            let (r, _) = ui.allocate_exact_size(vec2(260.0, 40.0), Sense::hover());
            if theme::button(ui, r, "pg-export", "Export Selected", Some(Icon::Export), Kind::Primary, can, 15.0).clicked() {
                app.export_selected(&ctx);
            }
        });
    });
    let rinner = theme::panel(ui, rr, "Last Task Details");
    theme::region(ui, rinner, "export-log", |ui| {
        if let Some(job) = &app.job {
            ui.label(RichText::new(format!("{}...  {:.0}%", job.label, job.progress() * 100.0)).color(ORANGE));
            ui.add(egui::ProgressBar::new(job.progress()).desired_width(ui.available_width()));
        }
        ui.label(RichText::new(&app.status).color(TEXT));
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let text = if app.last_log.is_empty() { "The FFmpeg command and any error messages from the last export appear here.".to_string() } else { app.last_log.clone() };
            ui.label(RichText::new(text).monospace().size(11.5).color(DIM));
        });
    });
}

// ---------------------------------------------------------------- settings page
pub fn settings_page(app: &mut Studio, ui: &mut Ui, body: Rect) {
    let ctx = ui.ctx().clone();
    let inner = theme::panel(ui, body, "Settings");
    theme::region(ui, inner, "settings", |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(900.0);
            heading(ui, "Output folder");
            ui.label(app.settings.output_dir.display().to_string());
            ui.horizontal(|ui| {
                if ui.button("Change...").clicked() {
                    app.change_output_folder(&ctx);
                }
                if ui.button("Open").clicked() {
                    app.open_folder(&app.settings.output_dir.clone());
                }
                if ui.button("Use Default (Videos\\Morrison Capture Studio)").clicked() {
                    app.settings.output_dir = crate::settings::default_output_dir();
                    app.settings.save();
                    app.library.refresh(&app.settings.output_dir.clone());
                }
            });
            heading(ui, "Recording");
            if ui.checkbox(&mut app.settings.keep_raw_tracks, "Keep the separate raw tracks (screen-only video and each WAV) in a \"Raw Tracks\" sub-folder").changed() {
                app.settings.save();
            }
            ui.label(RichText::new("Speakers, microphone, screen, cursor, click highlighting, quality and frame rate are set on the Record page and remembered automatically.").small().color(DIM));
            ui.horizontal(|ui| {
                ui.label("Custom region:");
                let [x, y, w, h] = app.settings.custom_region;
                ui.label(format!("{w} x {h} at {x}, {y}"));
                if ui.button("Change...").clicked() {
                    app.dialog = Dialog::Region;
                }
            });
            heading(ui, "Preview");
            ui.checkbox(&mut app.loop_play, "Loop playback");
            heading(ui, "Media engine (FFmpeg)");
            match &app.engine {
                Some(Ok(p)) => {
                    ui.label(RichText::new("Ready - built into this program, nothing to install.").color(GREEN));
                    ui.label(RichText::new(p.display().to_string()).small().color(DIM));
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(format!("Problem: {e}")).color(RED));
                }
                None => {
                    ui.label(RichText::new("Starting...").color(ORANGE));
                }
            }
            if ui.button("Open settings and log folder").clicked() {
                app.open_folder(&crate::settings::app_data_dir());
            }
            heading(ui, "Keyboard shortcuts");
            for (k, d) in [("Space", "Play / pause the preview"), ("Ctrl + Z", "Undo an edit"), ("Ctrl + Y", "Redo"), ("Esc", "Leave full-screen preview")] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).font(semi(13.0)));
                    ui.label(RichText::new(d).color(DIM));
                });
            }
            heading(ui, "About");
            ui.label(format!("Morrison Capture Studio v{}  -  portable Windows recorder and editor.", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new("Some apps block recording of their sound or picture (copy protection). Respect privacy and copyright when recording.").small().color(DIM));
        });
    });
}

// ---------------------------------------------------------------- dialogs
fn dialog_window(ctx: &egui::Context, title: &str) -> egui::Window<'static> {
    egui::Window::new(RichText::new(title.to_string()).font(semi(15.0)))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .default_width(420.0)
        .id(egui::Id::new(("dialog", title.to_string())))
        .order(egui::Order::Foreground)
        .frame(egui::Frame::window(&ctx.global_style()).fill(PANEL).inner_margin(16.0))
}

pub fn dialogs(app: &mut Studio, ctx: &egui::Context) {
    if app.dialog == Dialog::None {
        return;
    }
    // dim the background (not for Crop, so the preview can still be drawn on)
    let screen = ctx.content_rect();
    if !matches!(app.dialog, Dialog::Crop) {
    egui::Area::new(egui::Id::new("dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(120));
        let _ = resp;
    });
    }
    let mut close = false;
    let dur = app.duration();
    match app.dialog.clone() {
        Dialog::Trim => {
            dialog_window(ctx, "Trim").show(ctx, |ui| {
                ui.label("Choose the part to keep. You can also drag across the waveform.");
                ui.add_space(6.0);
                egui::Grid::new("trimg").num_columns(3).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Start");
                    ui.add(egui::DragValue::new(&mut app.edit.trim_start).range(0.0..=dur).speed(0.05).custom_formatter(|v, _| format!("{v:.2} s")));
                    if ui.button("Use Playhead").clicked() {
                        app.edit.trim_start = app.playhead;
                    }
                    ui.end_row();
                    ui.label("End");
                    ui.add(egui::DragValue::new(&mut app.edit.trim_end).range(0.0..=dur).speed(0.05).custom_formatter(|v, _| if v <= 0.0 { "end".into() } else { format!("{v:.2} s") }));
                    if ui.button("Use Playhead").clicked() {
                        app.edit.trim_end = app.playhead;
                    }
                    ui.end_row();
                });
                let (s, e) = app.edit.selection(dur);
                ui.label(RichText::new(format!("Keeping {} to {} ({}) of {}", clock(s), clock(e), clock(e - s), clock(dur))).color(DIM));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Save Trimmed Copy").strong()).clicked() {
                        let ed = EditState { trim_start: app.edit.trim_start, trim_end: app.edit.trim_end, ..Default::default() };
                        app.save_copy(ctx, "Trimmed", ed);
                        close = true;
                    }
                    if ui.button("Keep for Export").on_hover_text("Close and apply this trim when you press Export Selected").clicked() {
                        close = true;
                    }
                    if ui.button("Clear Trim").clicked() {
                        app.edit.trim_start = 0.0;
                        app.edit.trim_end = 0.0;
                    }
                });
            });
        }
        Dialog::Volume => {
            dialog_window(ctx, "Volume").show(ctx, |ui| {
                ui.label("Make the sound louder or quieter.");
                ui.add(egui::Slider::new(&mut app.edit.gain_db, -30.0..=24.0).step_by(0.5).suffix(" dB").text("Volume change"));
                ui.checkbox(&mut app.edit.normalize, "Normalize (automatic even loudness)");
                ui.label(RichText::new("+6 dB is about twice as loud, -6 dB about half.").small().color(DIM));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Save Copy with New Volume").strong()).clicked() {
                        let ed = EditState { gain_db: app.edit.gain_db, normalize: app.edit.normalize, ..Default::default() };
                        app.save_copy(ctx, "Volume", ed);
                        close = true;
                    }
                    if ui.button("Keep for Export").clicked() {
                        close = true;
                    }
                    if ui.button("Reset").clicked() {
                        app.edit.gain_db = 0.0;
                        app.edit.normalize = false;
                    }
                });
            });
        }
        Dialog::Crop => {
            let info = app.info.clone().unwrap_or_default();
            egui::Window::new(RichText::new("Crop").font(semi(15.0)))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, vec2(-24.0, 96.0))
                .default_width(330.0)
                .order(egui::Order::Foreground)
                .frame(egui::Frame::window(&ctx.global_style()).fill(PANEL).inner_margin(14.0))
                .show(ctx, |ui| {
                    ui.label("Drag a rectangle on the video preview, or type the numbers.");
                    let (w, h) = (info.width as i32, info.height as i32);
                    egui::Grid::new("cropg").num_columns(4).show(ui, |ui| {
                        ui.label("X");
                        ui.add(egui::DragValue::new(&mut app.edit.crop[0]).range(0..=w).speed(2));
                        ui.label("Y");
                        ui.add(egui::DragValue::new(&mut app.edit.crop[1]).range(0..=h).speed(2));
                        ui.end_row();
                        ui.label("Width");
                        ui.add(egui::DragValue::new(&mut app.edit.crop[2]).range(0..=w).speed(2));
                        ui.label("Height");
                        ui.add(egui::DragValue::new(&mut app.edit.crop[3]).range(0..=h).speed(2));
                        ui.end_row();
                    });
                    ui.label(RichText::new(format!("Video is {w} x {h}")).small().color(DIM));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let ok = app.edit.crop[2] > 8 && app.edit.crop[3] > 8;
                        if ui.add_enabled(ok, egui::Button::new(RichText::new("Save Cropped Copy").strong())).clicked() {
                            let ed = EditState { crop: app.edit.crop, ..Default::default() };
                            app.save_copy(ctx, "Cropped", ed);
                            close = true;
                        }
                        if ui.button("Keep for Export").clicked() {
                            close = true;
                        }
                        if ui.button("Clear").clicked() {
                            app.edit.crop = [0; 4];
                        }
                    });
                });
            if close {
                app.crop_mode = false;
            }
            // The crop dialog does not dim the screen so you can draw on the preview.
        }
        Dialog::Delete(p) => {
            dialog_window(ctx, "Delete File").show(ctx, |ui| {
                ui.label(format!("Move \"{}\" to the Recycle Bin?", edit::name(&p)));
                ui.label(RichText::new("You can restore it from the Recycle Bin later.").small().color(DIM));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Delete").color(RED).strong()).clicked() {
                        app.delete_file(&p, ctx);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Dialog::Rename(p, mut name) => {
            dialog_window(ctx, "Rename File").show(ctx, |ui| {
                ui.label("New name (the extension stays the same):");
                let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(360.0));
                r.request_focus();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button(RichText::new("Rename").strong()).clicked() || enter {
                        app.rename_file(&p, &name, ctx);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if !close {
                app.dialog = Dialog::Rename(p, name);
            }
        }
        Dialog::Region => {
            dialog_window(ctx, "Custom Recording Region").show(ctx, |ui| {
                ui.label("Area of the desktop to record, in screen pixels.");
                let r = &mut app.settings.custom_region;
                egui::Grid::new("regg").num_columns(4).show(ui, |ui| {
                    ui.label("Left (X)");
                    ui.add(egui::DragValue::new(&mut r[0]).speed(2));
                    ui.label("Top (Y)");
                    ui.add(egui::DragValue::new(&mut r[1]).speed(2));
                    ui.end_row();
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut r[2]).range(16..=16384).speed(2));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut r[3]).range(16..=16384).speed(2));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    for (l, w, h) in [("1920 x 1080", 1920, 1080), ("1280 x 720", 1280, 720), ("800 x 600", 800, 600)] {
                        if ui.small_button(l).clicked() {
                            r[2] = w;
                            r[3] = h;
                        }
                    }
                });
                let v = crate::win::virtual_screen();
                ui.label(RichText::new(format!("Your desktop spans {} x {} starting at {}, {}.", v.right - v.left, v.bottom - v.top, v.left, v.top)).small().color(DIM));
                ui.add_space(8.0);
                if ui.button(RichText::new("Done").strong()).clicked() {
                    close = true;
                }
            });
            if close {
                app.settings.save();
            }
        }
        Dialog::PreviewSettings => {
            dialog_window(ctx, "Preview Settings").show(ctx, |ui| {
                ui.checkbox(&mut app.loop_play, "Loop playback");
                let mut v = app.settings.preview_volume;
                if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text("Preview volume")).changed() {
                    app.set_volume(v);
                }
                ui.label(RichText::new("The preview always plays inside this window - no other player is opened. Edits are applied on export.").small().color(DIM));
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        }
        Dialog::CloseConfirm => {
            let recording = app.recording();
            dialog_window(ctx, if recording { "Recording in Progress" } else { "Task in Progress" }).show(ctx, |ui| {
                if recording {
                    ui.label("A recording is running. Stop and save it before closing?");
                } else {
                    ui.label("A task is still running (saving or exporting).");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let first = if recording { "Stop, Save and Close" } else { "Finish, then Close" };
                    if ui.button(RichText::new(first).strong()).clicked() {
                        app.stop_save_and_close(ctx);
                        close = true;
                    }
                    if ui.button("Close Without Saving").clicked() {
                        app.close_now(ctx);
                        close = true;
                    }
                    if ui.button("Keep Working").clicked() {
                        close = true;
                    }
                });
            });
        }
        Dialog::None => {}
    }
    if close {
        app.dialog = Dialog::None;
    }
}
