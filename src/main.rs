use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::fs;
use std::os::unix::fs::PermissionsExt;


fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if let Ok(_) = io::stdin().read_line(&mut input) {
            let input = input.trim();
            let mut parts = input.split_whitespace();
            let command = parts.next().unwrap_or("");
            let args = parts.collect::<Vec<&str>>();

            match command {
                "cd" => {
                    let new_dir = args.first().map_or("/", |x| *x);
                    let root = Path::new(new_dir);
                    if let Err(e) = env::set_current_dir(&root) {
                        eprintln!("{}", e);
                    }
                },
                "exit" => exit(0),
                "echo" => {
                    let echo_str = args.join(" ");
                    println!("{}", echo_str);
                },
                "pwd" => {
                    println!("{}", env::current_dir().unwrap().display());
                },
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
                },
                "ls" => {
                    let mut show_hidden = false;
                    let mut long_format = false;
                    let mut classify = false;

                    for arg in args {
                        if arg.starts_with("-") {
                            show_hidden |= arg.contains("a");
                            long_format |= arg.contains("l");
                            classify |= arg.contains("F");
                        }
                    }

                    list_directory(".", show_hidden, long_format, classify);
                },
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
                },
                "cp" => {
                    if args.len() != 2 {
                        eprintln!("cp: wrong number of arguments");
                    } else {
                        let source = Path::new(args[0]);
                        let destination = Path::new(args[1]);
                        if let Err(e) = copy_file(source, destination) {
                            eprintln!("cp: {}: {}", source.display(), e);
                        }
                    }
                },
                "mv" => {
                    if args.len() != 2 {
                        eprintln!("mv: wrong number of arguments");
                    } else {
                        let source = Path::new(args[0]);
                        let destination = Path::new(args[1]);
                        if let Err(e) = move_item(source, destination) {
                            eprintln!("mv: {}: {}", source.display(), e);
                        }
                    }
                },
                "mkdir" => {
                    if args.is_empty() {
                        eprintln!("mkdir: missing operand");
                    } else {
                        for dir_name in args {
                            let path = Path::new(dir_name);
                            match fs::create_dir(&path) {
                                Ok(_) => {},
                                Err(e) => eprintln!("mkdir: {}: {}", dir_name, e),
                            }
                        }
                    }
                },
                _ => eprintln!("{}: command not found", command),
            }
        } else {
            println!();
            exit(0); // Exit on Ctrl+D
        }
    }
}

fn list_directory(path: &str, show_hidden: bool, long_format: bool, classify: bool) {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("ls: cannot access '{}': {}", path, e);
            return;
        }
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let file_name = entry.file_name().to_string_lossy().into_owned();

            // Skip hidden files unless -a is specified
            if !show_hidden && file_name.starts_with('.') {
                continue;
            }

            if long_format {
                // Implement long format listing details (like permissions, owner, size)
                // This is a placeholder for actual implementation
                print!("drwxr-xr-x 1 user group 4096 Jan 1 00:00 ");
            }

            print!("{}", file_name);

            if classify {
                // Add a character indicating the file type
                let metadata = if let Ok(metadata) = entry.metadata() {
                    metadata
                } else {
                    continue;
                };

                if metadata.is_dir() {
                    print!("/");
                } else if metadata.permissions().mode() & 0o111 != 0 {
                    print!("*");
                }
                // Add more file types (like symbolic links) as needed
            }

            println!();
        }
    }
}

fn remove_item(path: &Path, recursive: bool) -> Result<(), String> {
    if path.is_dir() {
        if recursive {
            for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                remove_item(&entry.path(), recursive)?;
            }
            fs::remove_dir(path).map_err(|e| e.to_string())
        } else {
            Err(format!("{}: is a directory", path.display()))
        }
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if source.is_dir() {
        return Err(format!("'{}' is a directory", source.display()));
    }

    let destination = if destination.is_dir() {
        destination.join(source.file_name().ok_or_else(|| "Invalid file name".to_string())?)
    } else {
        destination.to_path_buf()
    };

    fs::copy(source, &destination).map(|_| ()).map_err(|e| e.to_string())
}

fn move_item(source: &Path, destination: &Path) -> Result<(), String> {
    let destination = if destination.is_dir() {
        destination.join(source.file_name().ok_or_else(|| "Invalid file name".to_string())?)
    } else {
        destination.to_path_buf()
    };

    fs::rename(source, &destination).map_err(|e| e.to_string())
}