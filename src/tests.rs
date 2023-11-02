use std::path::PathBuf;

use crate::{args::Args, generate_template};

const BASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");

#[test]
fn normal() {
    let args = Args {
        template: "normal".to_string(),
        ..Default::default()
    };

    let result = generate_template(&args, PathBuf::from(BASE_DIR), vec!["test", "normal"])
        .expect("generate_template");
    println!("{result}");
}

#[test]
#[ignore]
fn resolve_config_hierarchy() {}

#[test]
#[ignore]
fn config_in_subdir() {}

#[test]
#[ignore]
fn intermediate_without_config() {}

#[test]
#[ignore]
fn malformed_config() {}

#[test]
#[ignore]
fn malformed_template() {}

#[test]
#[ignore]
fn override_template() {}

#[test]
#[ignore]
fn template_at_path() {}

#[test]
#[ignore]
fn toplevel_setting() {}

mod args {
    #[test]
    #[ignore]
    fn nonexistent_file() {}

    #[test]
    #[ignore]
    fn omit_optional() {}

    #[test]
    #[ignore]
    fn config_in_subdir() {}

    #[test]
    #[ignore]
    fn bad_int() {}

    #[test]
    #[ignore]
    fn bad_float() {}

    #[test]
    #[ignore]
    fn omit_required_option() {}
}
