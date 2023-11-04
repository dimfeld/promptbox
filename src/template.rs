use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{error::Error, model::ModelOptionsInput};

#[derive(Deserialize, Debug, Default, Copy, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    #[default]
    String,
    Number,
    Integer,
    #[serde(alias = "boolean")]
    Bool,
    File,
}

#[derive(Deserialize, Debug)]
pub struct PromptOption {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub array: bool,
    #[serde(rename = "type", default)]
    pub option_type: OptionType,
    /// If this option is omitted, use this default value instead.
    /// Options without a default value and without `optional` are required.
    pub default: Option<liquid::model::Value>,
    /// Set `optional` true to allow omitting the option without providing a default value
    #[serde(default)]
    pub optional: bool,
}

#[derive(Deserialize, Debug)]
pub struct PromptTemplate {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub model: ModelOptionsInput,

    #[serde(default)]
    pub options: HashMap<String, PromptOption>,

    pub template: Option<String>,
    pub template_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ParsedTemplate {
    pub name: String,
    pub input: PromptTemplate,
    pub path: PathBuf,
    pub template: String,
}

impl ParsedTemplate {
    /// Try to load a template from a file. If the file does not exist, returns `Ok(None)`.
    pub fn from_file(name: &str, path: &Path) -> Result<Option<Self>, Report<Error>> {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return Ok(None);
        };

        let mut prompt_template: PromptTemplate = toml::from_str(&contents)
            .change_context(Error::ParseTemplate)
            .attach_printable_lazy(|| path.display().to_string())?;

        // At some point we should support partials here, but it still needs some design since we
        // want to allow templates to reference partials in upper directories. For now, we just
        // do a String.
        let (template_path, template_result) = if let Some(t) = prompt_template.template.take() {
            // Template is embedded in the file
            (path.to_path_buf(), t)
        } else {
            // Load it from the specified path
            let relative_template_path = prompt_template
                .template_path
                .as_ref()
                .ok_or(Error::EmptyTemplate)?;
            let template_path = path
                .parent()
                .ok_or(Error::EmptyTemplate)?
                .join(relative_template_path);

            let template_contents = std::fs::read_to_string(&template_path)
                .change_context(Error::TemplateContentsNotFound)
                .attach_printable_lazy(|| template_path.display().to_string())?;
            (template_path, template_contents)
        };

        Ok(Some(ParsedTemplate {
            name: name.to_string(),
            input: prompt_template,
            path: template_path,
            template: template_result,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        panic::Location,
        path::{Path, PathBuf},
    };

    use crate::generate_template;

    const BASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");

    fn base_dir(subpath: impl AsRef<Path>) -> PathBuf {
        PathBuf::from(BASE_DIR).join(subpath)
    }

    fn to_cmdline_vec(args: Vec<impl Into<OsString>>) -> Vec<OsString> {
        args.into_iter()
            .map(|arg| arg.into())
            .collect::<Vec<OsString>>()
    }

    #[test]
    fn normal() {
        let cmdline = to_cmdline_vec(vec![
            "test",
            "run",
            "normal",
            "--defaulttypeopt",
            "defvalue",
            "--defaultvalue",
            "5",
            "--stringopt",
            "stringvalue",
            "--numopt",
            "5.5",
            "--intopt",
            "6",
            "--boolopt",
            "--fileopt",
            "test1.txt",
            "--arrayopt",
            "array",
            "--arrayopt",
            "arrayb",
            "--arrayfileopt",
            "test1.txt",
            "--arrayfileopt",
            "test2.txt",
            "--optional",
            "optvalue",
        ]);

        let (args, options, prompt) =
            generate_template(PathBuf::from(BASE_DIR), "normal".to_string(), cmdline)
                .expect("generate_template");
        assert_eq!(
            prompt,
            r##"This is a template.

stringvalue 5.5 6
Single File test1.txt: test1
test1.txt: test1
test2.txt: it's test2
5
optvalue
"##
        );
    }

    #[test]
    fn malformed_template() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "malformed_template"]);

        let result = generate_template(
            base_dir("malformed_template"),
            "malformed_template".to_string(),
            cmdline,
        );
        error_stack::Report::install_debug_hook::<Location>(|_, _| {});
        let err = result.expect_err("aa");
        println!("{err:#?}");
    }

    #[test]
    #[ignore]
    fn override_template() {}

    #[test]
    #[ignore]
    fn template_at_path() {}

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
}
