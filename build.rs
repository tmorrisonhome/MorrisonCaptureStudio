fn main() {
    #[cfg(target_os="windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "Morrison Capture Studio");
        res.set("FileDescription", "Portable audio and screen capture editor");
        res.set("FileVersion", "2.0.0.0");
        let _ = res.compile();
    }
}
