mod bash_file;
mod config;
mod executables;
// mod history;
mod script;
mod shells;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use bash_file::BashFile;
use executables::Executables;
use fuzzy_finder::item::Item;
use fuzzy_finder::FuzzyFinder;
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use pastel_colours::{GREEN_FG, RED_FG, RESET_FG};
use script::Function;
use shells::UserShell;
use spinners::{Spinner, Spinners};
use structopt::StructOpt;
use tempfile::tempdir;
use ui::{print_bad_function_name, print_bad_script_name};

// use crate::history::History;
use crate::script::Script;

/// Use lk to explore and execute scripts in your current directory,
/// and in its sub-directories. lk offers two options: 'list' or 'fuzzy'.
/// 'list' lets you explore your scripts and their functions in a
/// hierarchical way. 'fuzzy' lets you do a fuzzy search over all the
/// scripts and functions found by lk.
#[derive(StructOpt)]
struct Cli {
    /// Set the default mode: fuzzy or list
    #[structopt(long, short)]
    default: Option<String>,
    /// Fuzzy search for available scripts and functions.
    #[structopt(long, short)]
    fuzzy: bool,
    /// List available scripts and functions.
    #[structopt(long, short)]
    list: bool,
    /// Optional: the name of a script to explore or use
    script: Option<String>,
    /// Optional: the name of the function to run.
    function: Option<String>,
    /// Optional: paths to ignore in the search
    #[structopt(long, short)]
    ignore: Vec<PathBuf>,
    /// Number of lines to show in fuzzy search
    #[structopt(long, short = "n", default_value = "7")]
    number: i8,
    /// Optional: params for the function. We're not processing them yet (e.g. validating) but
    /// they need to be permitted as a param to lk.
    #[allow(dead_code)]
    params: Vec<String>,
}

fn main() -> Result<()> {
    let lk_dir = match dirs::home_dir() {
        // Use a dir in ~/.config like a good human, but then store logs in it lol.
        Some(home_dir) => format!("{}/.config/lk", home_dir.to_string_lossy()),
        // If we don't have access to the home_dir for some reason then just use a temp dir.
        None => {
            println!("Unable to access your home directory. Using a temporary directory instead.");
            tempdir().unwrap().into_path().to_string_lossy().to_string()
        }
    };

    let mut config_file = config::ConfigFile::new(&lk_dir, "lk.toml");

    let args = Cli::from_args();

    let log_file_path = format!("{lk_dir}/lk.log");
    let log_file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build(&log_file_path)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(log_file)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;
    log4rs::init_config(config)?;

    log::info!("\n\nStarting lk...");

    let sp = Spinner::new(&Spinners::Line, "".to_string());
    let executables = Executables::new(
        ".",
        &args
            .ignore
            .iter()
            .map(|p| PathBuf::from(".").join(p))
            .collect::<Vec<_>>(),
    );
    sp.stop();

    let scripts: Vec<Script> = executables
        .executables
        .iter()
        .map(Script::new)
        .filter_map(Result::ok)
        .collect();

    // Prints all scripts
    // scripts.iter().for_each(|script| {
    //     script
    //         .functions
    //         .iter()
    //         .for_each(|function| println!("{} - {}", script.file_name(), function.name))
    // });
    if let Some(default) = args.default {
        match default.as_str() {
            "fuzzy" => {
                println!("Setting default mode to {GREEN_FG}fuzzy{RESET_FG}");
                config_file.config.default_mode = "fuzzy".to_string();
                config_file.save();
            }
            "list" => {
                println!("Setting default mode to {GREEN_FG}list{RESET_FG}");
                config_file.config.default_mode = "list".to_string();
                config_file.save();
            }
            _ => {
                println!(
                    "{RED_FG}Unknown default!{RESET_FG} Please specify either {GREEN_FG}fuzzy{RESET_FG} or {GREEN_FG}list{RESET_FG}. You can try out either using the {GREEN_FG}--fuzzy{RESET_FG} or {GREEN_FG}--list{RESET_FG} flags.",
                );
            }
        }
    } else if args.fuzzy {
        fuzzy(&scripts, args.number + 1)?
    } else if args.list || args.script.is_some() {
        // If the user is specifying --list OR if there's some value for script.
        // Any value there is implicitly take as --list.
        list(executables, args)?
    } else {
        // Neither requested, so fall back on the default which will always exist.
        match config_file.config.default_mode.as_str() {
            "fuzzy" => fuzzy(&scripts, args.number + 1)?,
            "list" => list(executables, args)?,
            _ => panic!("No default mode set! Has there been a problem creating the config file?"),
        }
    }
    Ok(())
}

/// Runs lk in 'fuzzy' mode.
fn fuzzy(scripts: &[Script], lines_to_show: i8) -> Result<()> {
    let result = FuzzyFinder::find(scripts_to_item(scripts), lines_to_show).unwrap();
    if let Some(function) = result {
        // We're going to write the equivelent lk command to the shell's history
        // file, so the user can easily re-run it.
        let history = UserShell::new();
        match history {
            Some(history) => {
                let lk_command = format!("lk {} {}", function.0.file_name(), function.1.name,);
                history.add_command(lk_command)?;
            }
            None => {
                log::warn!("Unable to write to history file because we couldn't figure out what shell you're using");
            }
        }
        // Finally we execute the function using a temporary bash file.
        BashFile::run(function.0.to_owned(), function.1.to_owned(), [].to_vec())?;
    }
    Ok(())
}

/// Runs lk in 'list' mode.
fn list(executables: Executables, args: Cli) -> Result<()> {
    // Did the user request a script?
    if let Some(script) = args.script {
        // Is it a script that exists on disk?
        if let Some(executable) = executables.get(&script) {
            // Yay, confirmed script
            let script = Script::new(executable)?;
            // Did the user pass a function?
            if let Some(function) = args.function {
                // Is it a function that exists in the script we found?
                if let Some(function) = script.get(&function) {
                    // Finally we execute the function using a temporary bash file.
                    BashFile::run(script.to_owned(), function.to_owned(), args.params)?;
                } else {
                    print_bad_function_name(&script, &function);
                }
            } else {
                // No function, display a list of what's available
                script.pretty_print();
            }
        } else {
            print_bad_script_name(&script, executables);
        }
    } else {
        // No executable, display a list of what's available
        executables.pretty_print();
    }
    Ok(())
}

/// Convert the scripts we find to the 'item' required for fuzzy find.
fn scripts_to_item(scripts: &[Script]) -> Vec<Item<(&Script, &Function)>> {
    let mut fuzzy_functions: Vec<Item<(&Script, &Function)>> = Vec::new();
    scripts.iter().for_each(|script| {
        script.functions.iter().for_each(|function| {
            fuzzy_functions.push(Item::new(
                format!(
                    "{}/{} - {}",
                    script.path(),
                    script.file_name(),
                    function.name
                ),
                (script, function),
            ))
        })
    });
    fuzzy_functions
}
