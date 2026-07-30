mod fileops;
mod glob;
mod ls;
mod parser;
mod users;

use fileops::{copy_file, move_item, remove_item};
use ls::{list_directory, list_directory_entry};
use parser::{parse_flags, parse_pipeline, tokenize, Redirect};
use std::env;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;

// Shared by the cp and mv handlers: a single source/destination pair
// behaves as before, but with more than one source the last argument
// must be an existing directory that every source gets copied/moved into.
fn copy_or_move_many(args: &[&str], label: &str, op: impl Fn(&Path, &Path) -> Result<(), String>) {
    if args.len() < 2 {
        eprintln!("{label}: missing file operand");
        return;
    }

    if args.len() == 2 {
        let source = Path::new(args[0]);
        let destination = Path::new(args[1]);
        if let Err(e) = op(source, destination) {
            eprintln!("{}: {}: {}", label, source.display(), e);
        }
        return;
    }

    let destination = Path::new(args[args.len() - 1]);
    if !destination.is_dir() {
        eprintln!(
            "{}: target '{}' is not a directory",
            label,
            destination.display()
        );
        return;
    }

    for source in &args[..args.len() - 1] {
        let source = Path::new(source);
        if let Err(e) = op(source, destination) {
            eprintln!("{}: {}: {}", label, source.display(), e);
        }
    }
}

// Runs a single command, writing its normal output to `output` (so callers
// can redirect it to a file or capture it for a pipeline) and any piped-in
// input via `input`. Errors always go to the real stderr, regardless of
// where `output` points, matching how redirection/piping normally only
// affects stdout.
fn execute_command(command: &str, args: &[&str], input: &str, output: &mut dyn Write) {
    match command {
        "cd" => {
            let new_dir = if let Some(dir) = args.first() {
                dir.to_string()
            } else {
                let Ok(home) = env::var("HOME") else {
                    eprintln!("cd: HOME not set");
                    return;
                };
                home
            };
            if let Err(e) = env::set_current_dir(Path::new(&new_dir)) {
                eprintln!("cd: {new_dir}: {e}");
            }
        }
        "exit" => exit(0),
        "echo" => {
            let echo_str = args.join(" ");
            let _ = writeln!(output, "{echo_str}");
        }
        "pwd" => {
            let _ = writeln!(output, "{}", env::current_dir().unwrap().display());
        }
        "cat" => {
            if args.is_empty() {
                if input.is_empty() {
                    eprintln!("cat: No file specified");
                } else {
                    let _ = write!(output, "{input}");
                }
            } else {
                for filename in args {
                    match std::fs::read_to_string(filename) {
                        Ok(contents) => {
                            let _ = write!(output, "{contents}");
                        }
                        Err(e) => eprintln!("cat: {filename}: {e}"),
                    }
                }
            }
        }
        "ls" => {
            let parsed_args = parse_flags(args);
            let long_format = parsed_args.contains(&"-l".to_string());
            let all = parsed_args.contains(&"-a".to_string());
            let classify = parsed_args.contains(&"-F".to_string());
            let paths: Vec<&String> = parsed_args.iter().filter(|a| !a.starts_with('-')).collect();

            if paths.is_empty() {
                list_directory(Path::new("."), long_format, all, classify, output);
            } else {
                let show_headers = paths.len() > 1;
                for (i, p) in paths.iter().enumerate() {
                    let path = Path::new(p.as_str());
                    match fs::metadata(path) {
                        Ok(metadata) if metadata.is_dir() => {
                            if show_headers {
                                if i > 0 {
                                    let _ = writeln!(output);
                                }
                                let _ = writeln!(output, "{p}:");
                            }
                            list_directory(path, long_format, all, classify, output);
                        }
                        Ok(metadata) => {
                            let _ = writeln!(
                                output,
                                "{}",
                                list_directory_entry(path, &metadata, classify, long_format)
                            );
                        }
                        Err(e) => eprintln!("ls: cannot access '{p}': {e}"),
                    }
                }
            }
        }
        "rm" => {
            let mut recursive = false;
            let mut files = Vec::new();

            for &arg in args {
                if arg == "-r" {
                    recursive = true;
                } else {
                    files.push(arg);
                }
            }

            if files.is_empty() {
                eprintln!("rm: missing operand");
            } else {
                for file in files {
                    let path = Path::new(file);
                    if let Err(e) = remove_item(path, recursive) {
                        eprintln!("rm: {file}: {e}");
                    }
                }
            }
        }
        "cp" => copy_or_move_many(args, "cp", copy_file),
        "mv" => copy_or_move_many(args, "mv", move_item),
        "mkdir" => {
            if args.is_empty() {
                eprintln!("mkdir: missing operand");
            } else {
                for &dir_name in args {
                    let path = Path::new(dir_name);
                    match fs::create_dir(path) {
                        Ok(()) => {}
                        Err(e) => eprintln!("mkdir: {dir_name}: {e}"),
                    }
                }
            }
        }
        _ => eprintln!("{command}: command not found"),
    }
}

// Runs a pipeline of one or more stages, feeding each stage's captured
// output to the next as `input`. The last stage writes to `redirect`'s
// target file if present, otherwise to real stdout.
fn run_pipeline(stages: &[Vec<String>], redirect: Option<&Redirect>) {
    let last_index = stages.len() - 1;
    let mut piped_input = String::new();

    for (i, stage) in stages.iter().enumerate() {
        let command = stage[0].as_str();
        let args: Vec<&str> = stage[1..].iter().map(String::as_str).collect();

        if i != last_index {
            let mut buffer: Vec<u8> = Vec::new();
            execute_command(command, &args, &piped_input, &mut buffer);
            piped_input = String::from_utf8_lossy(&buffer).into_owned();
            continue;
        }

        match redirect {
            None => {
                let mut stdout = io::stdout();
                execute_command(command, &args, &piped_input, &mut stdout);
            }
            Some(Redirect::Overwrite(filename)) => match File::create(filename) {
                Ok(mut file) => execute_command(command, &args, &piped_input, &mut file),
                Err(e) => eprintln!("{filename}: {e}"),
            },
            Some(Redirect::Append(filename)) => {
                match OpenOptions::new().create(true).append(true).open(filename) {
                    Ok(mut file) => execute_command(command, &args, &piped_input, &mut file),
                    Err(e) => eprintln!("{filename}: {e}"),
                }
            }
        }
    }
}

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap_or(0);
        if bytes_read == 0 {
            println!();
            exit(0); // Exit on Ctrl+D
        }

        let tokens = tokenize(input.trim());
        if tokens.is_empty() {
            continue;
        }
        let tokens = glob::expand_all(&tokens);

        match parse_pipeline(&tokens) {
            Ok((stages, redirect)) => run_pipeline(&stages, redirect.as_ref()),
            Err(e) => eprintln!("{e}"),
        }
    }
}
