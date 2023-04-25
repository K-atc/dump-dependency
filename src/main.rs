#![feature(exit_status_error)]

use clap::{Parser, Subcommand};
use log::error;
#[allow(unused_imports)]
use log::{info, trace, warn};
#[allow(unused_imports)]
use rayon::prelude::*;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Write;
#[allow(unused_imports)]
use std::io::{BufRead, Read};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatusError;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    compile_commands: String,
    #[clap(
        long = "exclude-system-headers",
        help = "Exclude system headers from dependency list"
    )]
    exclude_system_headers: bool,
    #[clap(long = "headers", help = "List only headers")]
    headers: bool,
    #[clap(subcommand)]
    command: CliSubCommand,
}

#[derive(Subcommand)]
enum CliSubCommand {
    List,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct CompileCommand {
    directory: PathBuf,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    arguments: Option<Vec<String>>,
    file: PathBuf,
}

#[derive(Debug)]
enum Error {
    IoError(io::Error),
    ExitStatusError(ExitStatusError),
    ShellWordsParseError(shell_words::ParseError),
    RegexError(regex::Error),
    CommandFormatError,
}

type Result<T> = std::result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IoError(error)
    }
}

impl From<ExitStatusError> for Error {
    fn from(error: ExitStatusError) -> Self {
        Error::ExitStatusError(error)
    }
}

impl From<shell_words::ParseError> for Error {
    fn from(error: shell_words::ParseError) -> Self {
        Error::ShellWordsParseError(error)
    }
}

impl From<regex::Error> for Error {
    fn from(error: regex::Error) -> Self {
        Error::RegexError(error)
    }
}

fn parse_dependency<R: Read>(output: BufReader<R>) -> Result<Vec<PathBuf>> {
    let re = Regex::new(r"\s*(.*) \\")?;
    let mut result = Vec::new();
    for line in output.lines() {
        if let Some(matches) = re.captures(&line?) {
            if let Some(path) = matches.get(1) {
                let path = Path::new(path.as_str());
                if path.exists() {
                    result.push(path.canonicalize()?);
                }
            }
        }
    }
    if result.is_empty() {
        warn!("No dependency found");
    }
    return Ok(result);
}

fn dump_dependency(command: &CompileCommand) -> Result<Vec<PathBuf>> {
    let mut args = if let Some(ref arguments) = command.arguments {
        arguments.clone()
    } else if let Some(ref command) = command.command {
        shell_words::split(command)?
    } else {
        return Err(Error::CommandFormatError);
    };
    assert_ne!(args.len(), 0);
    trace!("dump_dependency: args={:?}", args);

    #[derive(Debug)]
    struct ReplaceTargetOption {
        o: Option<usize>,
    }
    let replace_target_option: ReplaceTargetOption = (|args: &Vec<String>| -> ReplaceTargetOption {
        let o = args.iter().position(|v| v == &String::from("-o"));
        ReplaceTargetOption { o }
    })(&args);
    trace!(
        "dump_dependency: replace_target_option={:?}",
        replace_target_option
    );
    if let Some(o) = replace_target_option.o {
        args.remove(o + 1);
        args.remove(o);
    }

    args.insert(1, String::from("-M"));

    let output = Command::new(&args[0])
        .args(&args[1..])
        .current_dir(&command.directory)
        .output()?;
    if !output.stderr.is_empty() {
        // Tell human that a error occured
        let mut stdout = io::stdout().lock();
        stdout.write_all(&output.stderr)?;
    }
    output.status.exit_ok()?;
    assert_ne!(output.stdout.len(), 0);

    Ok(parse_dependency(BufReader::new(Cursor::new(
        output.stdout,
    )))?)
}

fn main() {
    env_logger::init();

    let args = Cli::parse();
    info!("args = {:?}", env::args());

    let compile_commands = fs::read_to_string(&args.compile_commands)
        .expect(format!("Failed to open file: {:?}", &args.compile_commands).as_str());
    let compile_commands: Vec<CompileCommand> =
        serde_json::from_str(&compile_commands).expect("Failed to parse");
    assert!(compile_commands.len() > 0);

    // Filter out commands for same file
    let compile_commands = {
        let mut unduplicated_compile_commands = Vec::new();
        let mut done_list = HashSet::new();
        for command in compile_commands.iter() {
            if done_list.contains(&command.file) {
                warn!(
                    "Another command for same file. Skip: file={:?}, arguments={:?}, command={:?}",
                    command.file, command.arguments, command.command
                );
                continue;
            }
            done_list.insert(&command.file);
            unduplicated_compile_commands.push(command);
        }
        unduplicated_compile_commands
    };

    let dependencies: Vec<_> = compile_commands
        .par_iter()
        .map(|command| {
            trace!("file={:?}", command.file);

            dump_dependency(command)
        })
        .collect();

    let mut dependency_list = HashSet::new();
    for dependency in dependencies {
        match dependency {
            Ok(ref paths) => {
                for v in paths.iter().cloned() {
                    if args.exclude_system_headers {
                        if v.starts_with("/usr") {
                            continue;
                        }
                    }
                    if args.headers {
                        if let Some(ext) = v.extension().and_then(OsStr::to_str) {
                            if !ext.starts_with("h") {
                                continue;
                            }
                        }
                    }
                    dependency_list.insert(v);
                }
            }
            Err(why) => {
                error!("{:?}", why)
            }
        }
    }
    let mut dependency_list: Vec<_> = dependency_list.iter().collect();
    dependency_list.sort();
    for path in dependency_list {
        println!("{}", path.display());
    }
}
