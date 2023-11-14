use std::{
    ffi::OsString,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
};

use args::{parse_main_args, parse_template_args, FoundCommand, GlobalRunArgs};
use config::Config;
use error::Error;
use error_stack::{Report, ResultExt};
use global_config::load_dotenv;
use liquid::partials::{InMemorySource, LazyCompiler};
use model::ModelOptions;
use template::ParsedTemplate;

use crate::model::send_model_request;

mod args;
mod config;
mod error;
mod global_config;
mod model;
mod ollama;
mod openai;
mod option;
mod template;
#[cfg(test)]
mod tests;

fn render_template(
    parser: &liquid::Parser,
    template_path: &Path,
    template: String,
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

fn generate_template(
    base_dir: PathBuf,
    template: String,
    cmdline: Vec<OsString>,
) -> Result<(GlobalRunArgs, ModelOptions, String, String), Report<Error>> {
    let config = Config::from_directory(base_dir.clone())?;

    let ParsedTemplate {
        template,
        path: template_path,
        input,
        system,
        ..
    } = config.find_template(&template)?;

    let (args, template_context) = parse_template_args(cmdline, &base_dir, &input)?;

    let template = match (args.prepend.as_ref(), args.append.as_ref()) {
        (Some(pre), Some(post)) => format!("{pre}\n\n{template}\n\n{post}"),
        (Some(pre), None) => format!("{pre}\n\n{template}"),
        (None, Some(post)) => format!("{template}\n\n{post}"),
        (None, None) => template,
    };

    let template = if args.extra_prompt.is_empty() {
        template
    } else {
        format!(
            "{template}\n\n{extra}",
            extra = args.extra_prompt.join("\n\n")
        )
    };

    let stdin = std::io::stdin();
    let template = if stdin.is_terminal() {
        // stdin is the terminal, so don't bother readying
        template
    } else {
        // Some text is potentially being piped in, so read it.
        let stdin_value = std::io::read_to_string(stdin)
            .attach_printable("Reading stdin")
            .change_context(Error::Io)?;
        if stdin_value.is_empty() {
            template
        } else {
            format!("{template}\n\n{stdin_value}")
        }
    };

    // TODO replace InMemorySource with a custom source that can look for partials in the various
    // config directories.
    let parser = liquid::ParserBuilder::<LazyCompiler<InMemorySource>>::default()
        .stdlib()
        .build()
        .expect("failed to build parser");

    let prompt = render_template(&parser, &template_path, template, &template_context)
        .attach_printable("Rendering template")
        .attach_printable_lazy(|| template_path.display().to_string())?;
    let system_prompt = if let Some((system_path, system_template)) = system {
        render_template(&parser, &system_path, system_template, &template_context)
            .attach_printable("Rendering system template")
            .attach_printable_lazy(|| system_path.display().to_string())?
    } else {
        String::new()
    };

    let mut model_options = config.model;
    model_options.update_from_args(&args);

    Ok((args, model_options, prompt, system_prompt))
}

fn run_template(
    base_dir: PathBuf,
    template: String,
    args: Vec<OsString>,
) -> Result<(), Report<Error>> {
    let (args, model_options, prompt, system) = generate_template(base_dir, template, args)?;

    if args.print_prompt || args.verbose || args.dry_run {
        if !system.is_empty() {
            println!("== System:\n{system}\n");
        }
        println!("== Prompt:\n{prompt}\n\n== Result:");
    }

    if args.dry_run {
        return Ok(());
    }

    let (message_tx, message_rx) = flume::bounded(32);
    let print_thread = std::thread::spawn(move || {
        let mut stdout = std::io::stdout();
        for message in message_rx {
            print!("{}", message);
            stdout.flush().ok();
        }

        println!("");
    });

    send_model_request(&model_options, &prompt, &system, message_tx)
        .change_context(Error::RunPrompt)?;

    print_thread.join().unwrap();

    Ok(())
}

fn run(base_dir: PathBuf, cmdline: Vec<OsString>) -> Result<(), Report<Error>> {
    let args = parse_main_args(cmdline).change_context(Error::ArgParseFailure)?;

    match args {
        FoundCommand::Run { template, args } => run_template(base_dir, template, args)?,
        FoundCommand::Other(cli) => {
            todo!()
        }
    }

    Ok(())
}

fn main() -> Result<(), Report<Error>> {
    // Don't show file locations in release mode
    #[cfg(not(debug_assertions))]
    error_stack::Report::install_debug_hook::<std::panic::Location>(|_, _| {});

    load_dotenv();
    run(
        std::env::current_dir().unwrap(),
        std::env::args().into_iter().map(OsString::from).collect(),
    )
}
