use std::{
    collections::HashMap,
    io::IsTerminal,
    path::{Path, PathBuf},
};

use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::{args::GlobalRunArgs, error::Error, model::ModelOptionsInput};

#[derive(Deserialize, Debug, Default, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    #[default]
    String,
    Number,
    #[serde(alias = "int")]
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

    pub system_prompt: Option<String>,
    pub system_prompt_path: Option<PathBuf>,

    pub template: Option<String>,
    pub template_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ParsedTemplate {
    pub name: String,
    pub input: PromptTemplate,
    pub path: PathBuf,
    pub template: String,
    pub system: Option<(PathBuf, String)>,
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

        let system = if let Some(t) = prompt_template.system_prompt.take() {
            // Template is embedded in the file
            Some((path.to_path_buf(), t))
        } else if let Some(relative_path) = prompt_template.system_prompt_path.as_ref() {
            // Load it from the specified path
            let template_path = path
                .parent()
                .ok_or(Error::EmptyTemplate)?
                .join(relative_path);

            let template_contents = std::fs::read_to_string(&template_path)
                .change_context(Error::TemplateContentsNotFound)
                .attach_printable_lazy(|| template_path.display().to_string())?;
            Some((template_path, template_contents))
        } else {
            None
        };

        Ok(Some(ParsedTemplate {
            name: name.to_string(),
            input: prompt_template,
            path: template_path,
            template: template_result,
            system,
        }))
    }
}

pub fn render_template(
    parser: &liquid::Parser,
    template_path: &Path,
    template: &str,
    context: &liquid::Object,
) -> Result<String, Report<Error>> {
    let parsed = parser
        .parse(&template)
        .change_context(Error::ParseTemplate)
        .attach_printable_lazy(|| template_path.display().to_string())?;

    let prompt = parsed
        .render(&context)
        .change_context(Error::ParseTemplate)
        .attach_printable_lazy(|| template_path.display().to_string())?;

    Ok(prompt)
}

pub fn template_references_extra(template: &str) -> bool {
    let extra_regex = regex::Regex::new(r##"\{\{-?\s*extra\s*-?\}\}"##).unwrap();
    extra_regex.is_match(template)
}

pub fn assemble_template(
    args: &mut GlobalRunArgs,
    template_context: &mut liquid::Object,
    initial_template: String,
) -> Result<String, Report<Error>> {
    let mut template = match args.prepend.as_ref() {
        Some(pre) => format!("{pre}\n\n{initial_template}"),
        None => initial_template,
    };

    let mut extra = std::mem::take(&mut args.extra_prompt);

    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        // Some text is potentially being piped in, so read it.
        let stdin_value = std::io::read_to_string(stdin)
            .attach_printable("Reading stdin")
            .change_context(Error::Io)?;
        if !stdin_value.is_empty() {
            extra.push(stdin_value);
        }
    };

    let extra_content = extra.join("\n\n");
    if template_references_extra(&template) {
        template_context.insert("extra".into(), liquid::model::Value::scalar(extra_content));
    } else if !extra_content.is_empty() {
        template = format!("{template}\n\n{extra_content}");
    }

    let template = match args.append.as_ref() {
        Some(append) => format!("{template}\n\n{append}"),
        None => template,
    };

    Ok(template)
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use super::ParsedTemplate;
    use crate::{
        error::Error,
        generate_template,
        tests::{base_dir, BASE_DIR},
    };

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

        let (_args, _options, prompt, system) =
            generate_template(PathBuf::from(BASE_DIR), "normal".to_string(), cmdline)
                .expect("generate_template");
        assert!(system.is_empty());
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
        let err = result.expect_err("should have been an error");
        println!("{err:#?}");
        assert!(matches!(err.current_context(), Error::ParseTemplate));
    }

    #[test]
    fn in_parent_dir() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "simple"]);

        let (_, _, prompt, _) =
            generate_template(base_dir("config_in_subdir"), "simple".to_string(), cmdline)
                .expect("generate_template");

        assert_eq!(prompt, "a simple prompt");
    }

    #[test]
    fn override_template() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "tmp"]);

        let (_, _, prompt, _) = generate_template(
            base_dir("override_template/override"),
            "tmp".to_string(),
            cmdline,
        )
        .expect("generate_template");

        assert_eq!(prompt, "overridden");
    }

    #[test]
    fn template_at_path() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "subdir_without_config/indir"]);

        let (_, _, prompt, _) = generate_template(
            PathBuf::from(BASE_DIR),
            "subdir_without_config/indir".to_string(),
            cmdline,
        )
        .expect("generate_template");

        assert_eq!(prompt, "the subdir");
    }

    #[test]
    fn nonexistent_file() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "nonexistent_file"]);
        let err = generate_template(
            PathBuf::from(BASE_DIR),
            "nonexistent_file".to_string(),
            cmdline,
        )
        .expect_err("generate_template");

        assert!(matches!(err.current_context(), Error::TemplateNotFound));
    }

    #[test]
    fn template_path_missing() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "missing_template_path"]);
        let err = generate_template(
            PathBuf::from(BASE_DIR),
            "missing_template_path".to_string(),
            cmdline,
        )
        .expect_err("generate_template");

        assert!(matches!(
            err.current_context(),
            Error::TemplateContentsNotFound
        ));
    }

    #[test]
    fn all_model_options() {
        let template = ParsedTemplate::from_file(
            "all_model_options",
            &base_dir(&PathBuf::from("all_model_options.pb.toml")),
        )
        .expect("loads successfully")
        .expect("should find template");

        let options = template.input.model;

        assert_eq!(options.model, Some("abc".to_string()));
        assert_eq!(
            options.lm_studio_host,
            Some("http://localhost:9998".to_string())
        );
        assert_eq!(
            options.ollama_host,
            Some("http://localhost:9999".to_string())
        );
        assert_eq!(options.temperature, Some(0.3));
        assert_eq!(options.format, Some(crate::model::OutputFormat::JSON));
        assert_eq!(options.top_p, Some(0.5));
        assert_eq!(options.top_k, Some(2));
        assert_eq!(options.frequency_penalty, Some(1.5));
        assert_eq!(options.presence_penalty, Some(0.5));
        assert_eq!(options.stop, Some(vec!["a".to_string(), "b".to_string()]));
        assert_eq!(options.max_tokens, Some(30));

        let mut aliases = options.alias.iter().collect::<Vec<_>>();
        aliases.sort();
        assert_eq!(
            aliases,
            vec![
                (&"llama2".to_string(), &"llama2:456".to_string()),
                (&"mistral".to_string(), &"mistral:123".to_string()),
            ]
        );

        assert_eq!(options.context.limit, Some(384));
        assert_eq!(options.context.reserve_output, Some(12));
        assert_eq!(
            options.context.keep,
            Some(crate::context::OverflowKeep::End)
        );
        assert_eq!(
            options.context.trim_args,
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            options.context.array_priority,
            Some(crate::context::ArrayTrimPriority::Equal)
        );
    }

    #[test]
    fn system_prompt() {
        let cmdline = to_cmdline_vec(vec!["test", "run", "system_prompt", "--type", "fruit"]);
        let (_, _, _, system_prompt) = generate_template(
            PathBuf::from(BASE_DIR),
            "system_prompt".to_string(),
            cmdline,
        )
        .expect("generate_template");

        assert_eq!(system_prompt, "a system prompt for fruit");
    }

    #[test]
    fn system_prompt_in_file() {
        let cmdline = to_cmdline_vec(vec![
            "test",
            "run",
            "system_prompt_in_file",
            "--type",
            "fruit",
        ]);
        let (_, _, _, system_prompt) = generate_template(
            PathBuf::from(BASE_DIR),
            "system_prompt_in_file".to_string(),
            cmdline,
        )
        .expect("generate_template");

        assert_eq!(system_prompt, "A system prompt for fruit\n");
    }

    mod assemble_template {
        use super::*;

        #[test]
        fn append() {
            let cmdline = to_cmdline_vec(vec![
                "test",
                "run",
                "normal",
                "--post",
                "Do it right",
                "Do it now",
                "Do it best",
            ]);

            let (_, _, prompt, _) =
                generate_template(PathBuf::from(BASE_DIR), "simple".to_string(), cmdline)
                    .expect("generate_template");
            assert_eq!(
                "a simple prompt\n\nDo it now\n\nDo it best\n\nDo it right",
                prompt
            );
        }

        #[test]
        fn prepend() {
            let cmdline = to_cmdline_vec(vec![
                "test",
                "run",
                "normal",
                "--pre",
                "Do it right",
                "Do it now",
                "Do it best",
            ]);

            let (_, _, prompt, _) =
                generate_template(PathBuf::from(BASE_DIR), "simple".to_string(), cmdline)
                    .expect("generate_template");
            assert_eq!(
                "Do it right\n\na simple prompt\n\nDo it now\n\nDo it best",
                prompt
            );
        }

        #[test]
        fn append_prepend() {
            let cmdline = to_cmdline_vec(vec![
                "test",
                "run",
                "normal",
                "--pre",
                "Do it right",
                "--post",
                "Is it done?",
                "Do it now",
                "Do it best",
            ]);

            let (_, _, prompt, _) =
                generate_template(PathBuf::from(BASE_DIR), "simple".to_string(), cmdline)
                    .expect("generate_template");
            assert_eq!(
                "Do it right\n\na simple prompt\n\nDo it now\n\nDo it best\n\nIs it done?",
                prompt
            );
        }

        #[test]
        fn extra_reference_in_template() {
            let cmdline = to_cmdline_vec(vec![
                "test",
                "run",
                "normal",
                "--pre",
                "Do it right",
                "--post",
                "Is it done?",
                "Do it now",
                "Do it best",
            ]);

            let (_, _, prompt, _) = generate_template(
                PathBuf::from(BASE_DIR),
                "extra_template_arg".to_string(),
                cmdline,
            )
            .expect("generate_template");
            assert_eq!(
                "Do it right\n\nSome text\n\nDo it now\n\nDo it best\n\nAnother\n\nIs it done?",
                prompt
            );
        }
    }

    mod args {
        use super::*;

        #[test]
        fn omit_optional() {
            let cmdline = to_cmdline_vec(vec![
                "test",
                "run",
                "normal",
                "--defaulttypeopt",
                "defvalue",
                "--stringopt",
                "stringvalue",
                "--numopt",
                "5.5",
                "--intopt",
                "6",
                "--fileopt",
                "test1.txt",
                "--arrayfileopt",
                "test1.txt",
            ]);

            let (_, _, prompt, _) =
                generate_template(PathBuf::from(BASE_DIR), "normal".to_string(), cmdline)
                    .expect("generate_template");
            assert_eq!(
                prompt,
                r##"This is a template.

stringvalue 5.5 6
Single File test1.txt: test1
test1.txt: test1
10

"##
            );
        }

        #[test]
        fn bad_int() {
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
                "6.5",
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

            let result = generate_template(base_dir("normal"), "normal".to_string(), cmdline);
            let err = result.expect_err("should have been an error");
            println!("{err:#?}");
            assert!(matches!(
                err.current_context(),
                Error::CmdlineParseFailure(_)
            ));
        }

        #[test]
        fn bad_float() {
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
                "abc",
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

            let result = generate_template(base_dir("normal"), "normal".to_string(), cmdline);
            println!("{result:#?}");
            let err = result.expect_err("should have been an error");
            println!("{err:#?}");
            assert!(matches!(
                err.current_context(),
                Error::CmdlineParseFailure(_)
            ));
        }

        #[test]
        fn omit_required_option() {
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

            let result = generate_template(base_dir("normal"), "normal".to_string(), cmdline);
            let err = result.expect_err("should have been an error");
            println!("{err:#?}");
            assert!(matches!(
                err.current_context(),
                Error::CmdlineParseFailure(_)
            ));
        }

        #[test]
        fn omit_required_array_option() {
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
                "--optional",
                "optvalue",
            ]);

            let result = generate_template(base_dir("normal"), "normal".to_string(), cmdline);
            let err = result.expect_err("should have been an error");
            println!("{err:#?}");
            assert!(matches!(
                err.current_context(),
                Error::CmdlineParseFailure(_)
            ));
        }
    }

    mod template_references_extra {
        use super::super::template_references_extra;

        #[test]
        fn basic() {
            assert_eq!(template_references_extra(" {{extra}} "), true);
        }

        #[test]
        fn spaces() {
            assert_eq!(template_references_extra("{{ extra }}"), true);
        }

        #[test]
        fn dashes() {
            assert_eq!(template_references_extra("{{-extra-}}"), true);
        }

        #[test]
        fn dashes_and_spaces() {
            assert_eq!(template_references_extra("{{- extra -}}"), true);
        }

        #[test]
        fn newlines() {
            assert_eq!(template_references_extra("{{\n\n\textra\n\t}}"), true);
        }

        #[test]
        fn notmatch_braces() {
            assert_eq!(template_references_extra("{extra}}"), false);
        }

        #[test]
        fn notmatch() {
            assert_eq!(template_references_extra("{{bextra}}"), false);
        }
    }
}
