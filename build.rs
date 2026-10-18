// Embeds the program icon and version details into the Windows EXE.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    // Check the *target* OS (not the machine doing the build).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // Resource compilation needs the Windows SDK tools; only run it when building on Windows.
    if !cfg!(windows) {
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "Morrison Capture Studio");
    res.set("FileDescription", "Morrison Capture Studio - audio and screen recorder and editor");
    res.set("ProductVersion", version);
    res.set("FileVersion", version);
    res.set("CompanyName", "Morrison");
    if let Err(e) = res.compile() {
        // Show the problem in the build log instead of silently ignoring it.
        println!("cargo:warning=Could not embed EXE icon/version info: {e}");
    }
}
