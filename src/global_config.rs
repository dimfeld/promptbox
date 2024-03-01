use std::path::PathBuf;

use etcetera::BaseStrategy;
use itertools::Itertools;

pub fn global_config_dirs() -> Vec<PathBuf> {
    let etc = etcetera::base_strategy::choose_native_strategy().unwrap();
    vec![etc.config_dir(), etc.home_dir().join(".config")]
        .into_iter()
        .unique()
        .map(|p| p.join("promptbox"))
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>()
}

pub fn load_dotenv() {
    dotenvy::dotenv().ok();
    for config_dir in global_config_dirs() {
        dotenvy::from_filename(config_dir.join(".env")).ok();
    }
}
