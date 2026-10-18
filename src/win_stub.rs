//! Non-Windows stand-in for win.rs (design preview build only).
#![allow(dead_code)]
use std::path::Path;
#[derive(Default, Clone, Copy, Debug)]
pub struct RECT { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
#[derive(Clone, Debug)]
pub struct Monitor { pub rect: RECT, pub primary: bool }
impl Monitor {
    pub fn width(&self) -> i32 { self.rect.right - self.rect.left }
    pub fn height(&self) -> i32 { self.rect.bottom - self.rect.top }
}
pub fn make_dpi_aware() {}
pub fn monitors() -> Vec<Monitor> { vec![Monitor { rect: RECT { left: 0, top: 0, right: 1920, bottom: 1080 }, primary: true }] }
pub fn virtual_screen() -> RECT { RECT { left: 0, top: 0, right: 1920, bottom: 1080 } }
pub fn left_button_down() -> bool { false }
pub fn cursor_pos() -> Option<(i32, i32)> { None }
pub fn local_stamp() -> String { "2026-10-18 15-30-00".into() }
pub fn recycle(path: &Path) -> Result<(), String> { std::fs::remove_file(path).map_err(|e| e.to_string()) }
