//! Small set of Windows API calls used by the app (monitors, mouse, local time, recycle bin).
#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::path::Path;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MONITORINFOEXW {
    cbSize: u32,
    rcMonitor: RECT,
    rcWork: RECT,
    dwFlags: u32,
    szDevice: [u16; 32],
}

#[repr(C)]
#[derive(Default)]
struct SYSTEMTIME {
    wYear: u16,
    wMonth: u16,
    wDayOfWeek: u16,
    wDay: u16,
    wHour: u16,
    wMinute: u16,
    wSecond: u16,
    wMilliseconds: u16,
}

#[repr(C)]
struct SHFILEOPSTRUCTW {
    hwnd: isize,
    wFunc: u32,
    pFrom: *const u16,
    pTo: *const u16,
    fFlags: u16,
    fAnyOperationsAborted: i32,
    hNameMappings: *mut std::ffi::c_void,
    lpszProgressTitle: *const u16,
}

type MonitorEnumProc = unsafe extern "system" fn(isize, isize, *mut RECT, isize) -> i32;

#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vkey: i32) -> i16;
    fn GetCursorPos(point: *mut POINT) -> i32;
    fn EnumDisplayMonitors(hdc: isize, clip: *const RECT, proc_: MonitorEnumProc, data: isize) -> i32;
    fn GetMonitorInfoW(monitor: isize, info: *mut MONITORINFOEXW) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(time: *mut SYSTEMTIME);
}
#[link(name = "shell32")]
extern "system" {
    fn SHFileOperationW(op: *mut SHFILEOPSTRUCTW) -> i32;
}

/// Make coordinates real screen pixels on scaled (125%/150%) displays.
/// Must run before the window is created.
pub fn make_dpi_aware() {
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = -4
    unsafe {
        SetProcessDpiAwarenessContext(-4);
    }
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub rect: RECT,
    pub primary: bool,
}
impl Monitor {
    pub fn width(&self) -> i32 {
        self.rect.right - self.rect.left
    }
    pub fn height(&self) -> i32 {
        self.rect.bottom - self.rect.top
    }
}

unsafe extern "system" fn enum_proc(hmon: isize, _hdc: isize, _r: *mut RECT, data: isize) -> i32 {
    let list = &mut *(data as *mut Vec<Monitor>);
    let mut info = MONITORINFOEXW {
        cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
        rcMonitor: RECT::default(),
        rcWork: RECT::default(),
        dwFlags: 0,
        szDevice: [0; 32],
    };
    if GetMonitorInfoW(hmon, &mut info) != 0 {
        list.push(Monitor { rect: info.rcMonitor, primary: info.dwFlags & 1 != 0 });
    }
    1
}

/// All monitors, primary first. Coordinates are physical desktop pixels.
pub fn monitors() -> Vec<Monitor> {
    let mut list: Vec<Monitor> = Vec::new();
    unsafe {
        EnumDisplayMonitors(0, std::ptr::null(), enum_proc, &mut list as *mut _ as isize);
    }
    list.sort_by_key(|m| (!m.primary, m.rect.left, m.rect.top));
    list
}

/// The whole desktop across every monitor.
pub fn virtual_screen() -> RECT {
    unsafe {
        let x = GetSystemMetrics(76); // SM_XVIRTUALSCREEN
        let y = GetSystemMetrics(77);
        let w = GetSystemMetrics(78);
        let h = GetSystemMetrics(79);
        RECT { left: x, top: y, right: x + w, bottom: y + h }
    }
}

pub fn left_button_down() -> bool {
    unsafe { GetAsyncKeyState(0x01) < 0 }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    if unsafe { GetCursorPos(&mut p) } != 0 {
        Some((p.x, p.y))
    } else {
        None
    }
}

/// Local date-time text such as "2026-10-18 14-30-25" (safe for file names).
pub fn local_stamp() -> String {
    let mut t = SYSTEMTIME::default();
    unsafe { GetLocalTime(&mut t) };
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Sends a file to the Recycle Bin (so a mistaken delete can be undone).
pub fn recycle(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.push(0);
    from.push(0); // list must end with two NULs
    let mut op = SHFILEOPSTRUCTW {
        hwnd: 0,
        wFunc: 3, // FO_DELETE
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: 0x0040 | 0x0010 | 0x0004 | 0x0400, // ALLOWUNDO | NOCONFIRMATION | SILENT | NOERRORUI
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let r = unsafe { SHFileOperationW(&mut op) };
    if r == 0 && op.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        // Fall back to a permanent delete if the Recycle Bin is unavailable (e.g. USB drive).
        std::fs::remove_file(path).map_err(|e| format!("Could not delete file: {e}"))
    }
}
