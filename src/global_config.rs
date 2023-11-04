use std::path::PathBuf;

use itertools::Itertools;

pub fn global_config_dirs() -> Vec<PathBuf> {
    vec![
        dirs::config_dir(),
        dirs::home_dir().map(|p| p.join(".config")),
    ]
    .into_iter()
    .flatten()
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
