mod fileops;
mod ls;
mod parser;
mod users;

use fileops::{copy_file, move_item, remove_item};
use ls::{list_directory, list_directory_entry};
use parser::{parse_flags, tokenize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;

// Shared by the cp and mv handlers: a single source/destination pair
// behaves as before, but with more than one source the last argument
// must be an existing directory that every source gets copied/moved into.
fn copy_or_move_many(args: &[&str], label: &str, op: impl Fn(&Path, &Path) -> Result<(), String>) {
    if args.len() < 2 {
        eprintln!("{}: missing file operand", label);
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

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        let bytes_read = io::stdin().read_line(&mut input).unwrap_or(0);
        if bytes_read > 0 {
            let tokens = tokenize(input.trim());
            let command = tokens.first().map(|s| s.as_str()).unwrap_or("");
            let args: Vec<&str> = tokens.iter().skip(1).map(|s| s.as_str()).collect();

            match command {
                "cd" => {
                    let new_dir = match args.first() {
                        Some(dir) => dir.to_string(),
                        None => match env::var("HOME") {
                            Ok(home) => home,
                            Err(_) => {
                                eprintln!("cd: HOME not set");
                                continue;
                            }
                        },
                    };
                    if let Err(e) = env::set_current_dir(Path::new(&new_dir)) {
                        eprintln!("cd: {}: {}", new_dir, e);
                    }
                }
                "exit" => exit(0),
                "echo" => {
                    let echo_str = args.join(" ");
                    println!("{}", echo_str);
                }
                "pwd" => {
                    println!("{}", env::current_dir().unwrap().display());
                }
                "cat" => {
                    if args.is_empty() {
                        eprintln!("cat: No file specified");
                    } else {
                        for filename in args {
                            match std::fs::read_to_string(filename) {
                                Ok(contents) => print!("{}", contents),
                                Err(e) => eprintln!("cat: {}: {}", filename, e),
                            }
                        }
                    }
                }
                "ls" => {
                    let parsed_args = parse_flags(&args);
                    let long_format = parsed_args.contains(&"-l".to_string());
                    let all = parsed_args.contains(&"-a".to_string());
                    let classify = parsed_args.contains(&"-F".to_string());
                    let paths: Vec<&String> =
                        parsed_args.iter().filter(|a| !a.starts_with('-')).collect();

                    if paths.is_empty() {
                        list_directory(Path::new("."), long_format, all, classify);
                    } else {
                        let show_headers = paths.len() > 1;
                        for (i, p) in paths.iter().enumerate() {
                            let path = Path::new(p.as_str());
                            match fs::metadata(path) {
                                Ok(metadata) if metadata.is_dir() => {
                                    if show_headers {
                                        if i > 0 {
                                            println!();
                                        }
                                        println!("{}:", p);
                                    }
                                    list_directory(path, long_format, all, classify);
                                }
                                Ok(metadata) => {
                                    println!(
                                        "{}",
                                        list_directory_entry(
                                            path,
                                            &metadata,
                                            classify,
                                            long_format
                                        )
                                    );
                                }
                                Err(e) => eprintln!("ls: cannot access '{}': {}", p, e),
                            }
                        }
                    }
                }
                "rm" => {
                    let mut recursive = false;
                    let mut files = Vec::new();

                    for arg in args {
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
                                eprintln!("rm: {}: {}", file, e);
                            }
                        }
                    }
                }
                "cp" => copy_or_move_many(&args, "cp", copy_file),
                "mv" => copy_or_move_many(&args, "mv", move_item),
                "mkdir" => {
                    if args.is_empty() {
                        eprintln!("mkdir: missing operand");
                    } else {
                        for dir_name in args {
                            let path = Path::new(dir_name);
                            match fs::create_dir(path) {
                                Ok(_) => {}
                                Err(e) => eprintln!("mkdir: {}: {}", dir_name, e),
                            }
                        }
                    }
                }
                _ => eprintln!("{}: command not found", command),
            }
        } else {
            println!();
            exit(0); // Exit on Ctrl+D
        }
    }
}
