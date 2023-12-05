use std::{ffi::OsString, path::PathBuf};

use args::{parse_main_args, parse_template_args, FoundCommand, GlobalRunArgs};
use config::Config;
use error::Error;
use error_stack::{Report, ResultExt};
use global_config::load_dotenv;
use liquid::partials::{InMemorySource, LazyCompiler};
use model::ModelOptions;
use template::{assemble_template, render_template, ParsedTemplate};

mod args;
mod config;
mod context;
mod error;
mod global_config;
mod hosts;
mod model;
mod option;
mod requests;
mod template;
#[cfg(test)]
mod tests;

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

    let (mut args, mut template_context) = parse_template_args(cmdline, &base_dir, &input)?;

    let mut model_options = config.model;
    model_options.update_from_model_input(&input.model);
    model_options.update_from_args(&args);

    let template = assemble_template(&mut args, &mut template_context, template)?;

    // TODO replace InMemorySource with a custom source that can look for partials in the various
    // config directories.
    let parser = liquid::ParserBuilder::<LazyCompiler<InMemorySource>>::default()
        .stdlib()
        .build()
        .expect("failed to build parser");

    let prompt = render_template(&parser, &template_path, &template, &template_context)
        .attach_printable("Rendering template")
        .attach_printable_lazy(|| template_path.display().to_string())?;
    let system_prompt = if let Some((system_path, system_template)) = system {
        render_template(&parser, &system_path, &system_template, &template_context)
            .attach_printable("Rendering system template")
            .attach_printable_lazy(|| system_path.display().to_string())?
    } else {
        String::new()
    };

    let prompt = context::enforce_context_limit(
        &model_options,
        &parser,
        &template_path,
        &template,
        template_context,
        prompt,
    )?;

    Ok((args, model_options, prompt, system_prompt))
}

fn run_template(
    base_dir: PathBuf,
    template: String,
    args: Vec<OsString>,
    mut output: impl std::io::Write + Send + 'static,
) -> Result<(), Report<Error>> {
    let (args, model_options, prompt, system) = generate_template(base_dir, template, args)?;

    if args.verbose {
        eprintln!("{model_options:?}");
    }

    if args.print_prompt || args.verbose || args.dry_run {
        if !system.is_empty() {
            eprintln!("== System:\n{system}\n");
        }
        eprintln!("== Prompt:\n{prompt}\n\n== Result:");
    }

    if args.dry_run {
        return Ok(());
    }

    let (message_tx, message_rx) = flume::bounded(32);
    let print_thread = std::thread::spawn(move || {
        for message in message_rx {
            write!(output, "{}", message)?;
            output.flush()?;
        }

        writeln!(output, "")?;
        Ok::<(), std::io::Error>(())
    });

    let system = if system.is_empty() {
        None
    } else {
        Some(system)
    };

    let host = model_options.api_host()?;
    host.send_model_request(&model_options, &prompt, system.as_deref(), message_tx)
        .change_context(Error::RunPrompt)?;

    print_thread.join().unwrap().ok();

    Ok(())
}

fn run(base_dir: PathBuf, cmdline: Vec<OsString>) -> Result<(), Report<Error>> {
    let args = parse_main_args(cmdline).map_err(Error::CmdlineParseFailure)?;

    match args {
        FoundCommand::Run { template, args } => {
            let stdout = std::io::stdout();
            run_template(base_dir, template, args, stdout)?;
        }
        FoundCommand::Other(_cli) => {
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
