use std::path::{Path, PathBuf};

pub const BASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");

pub fn base_dir(subpath: impl AsRef<Path>) -> PathBuf {
    PathBuf::from(BASE_DIR).join(subpath)
}
