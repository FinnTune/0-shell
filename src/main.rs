use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use users::{get_user_by_uid, get_group_by_gid};
use chrono::{DateTime, Local};


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
                    let long_format = args.contains(&"-l");
                    let all = args.contains(&"-a");
                    let classify = args.contains(&"-F");
                    list_directory(Path::new("."), long_format, all, classify);
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

fn list_directory_entry(path: &Path, metadata: &Metadata, classify: bool) -> String {
    let file_type = metadata.file_type();
    let permissions = metadata.permissions();

    let file_type_indicator = if file_type.is_dir() { "d" } else { "-" };
    let permissions_str = format!("{:o}", permissions.mode() & 0o777);

    let num_links = metadata.nlink();
    let owner = get_user_by_uid(metadata.uid()).map(|u| u.name().to_string_lossy().into_owned()).unwrap_or_else(|| metadata.uid().to_string());
    let group = get_group_by_gid(metadata.gid()).map(|g| g.name().to_string_lossy().into_owned()).unwrap_or_else(|| metadata.gid().to_string());
    let size = metadata.len();
    
    let datetime: DateTime<Local> = metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now()).into();
    let datetime_str = datetime.format("%b %d %H:%M").to_string();

    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let classify_char = if classify {
        if file_type.is_dir() { "/" } else if file_type.is_symlink() { "@" } else if permissions.mode() & 0o111 != 0 { "*" } else { "" }
    } else { "" };

    format!("{}{} {} {} {} {} {} {}", file_type_indicator, permissions_str, num_links, owner, group, size, datetime_str, name + classify_char)
}

fn list_directory(dir: &Path, long_format: bool, all: bool, classify: bool) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("ls: cannot access '{}': {}", dir.display(), e);
            return;
        }
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if !all && path.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                continue;
            }

            let metadata = match path.metadata() {
                Ok(metadata) => metadata,
                Err(e) => {
                    eprintln!("ls: cannot access '{}': {}", path.display(), e);
                    continue;
                }
            };

            let display_str = if long_format {
                list_directory_entry(&path, &metadata, classify)
            } else {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let classify_char = if classify {
                    if metadata.file_type().is_dir() { "/" } else if metadata.file_type().is_symlink() { "@" } else if metadata.permissions().mode() & 0o111 != 0 { "*" } else { "" }
                } else { "" };
                format!("{}{}", name, classify_char)
            };

            println!("{}", display_str);
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