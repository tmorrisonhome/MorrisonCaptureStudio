//! Stand-ins used ONLY for the non-Windows design-preview build (feature "preview").
//! The real Windows program never compiles this file.
#![allow(dead_code)]

pub mod rfd {
    use std::path::{Path, PathBuf};
    pub struct FileDialog;
    impl FileDialog {
        pub fn new() -> Self { Self }
        pub fn add_filter(self, _: &str, _: &[impl ToString]) -> Self { self }
        pub fn set_title(self, _: &str) -> Self { self }
        pub fn set_directory<P: AsRef<Path>>(self, _: P) -> Self { self }
        pub fn set_file_name(self, _: &str) -> Self { self }
        pub fn pick_file(self) -> Option<PathBuf> { None }
        pub fn pick_files(self) -> Option<Vec<PathBuf>> { None }
        pub fn save_file(self) -> Option<PathBuf> { None }
        pub fn pick_folder(self) -> Option<PathBuf> { None }
    }
}
