use chrono::TimeZone;
use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::fs::Metadata;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::exit;
use users::{get_group_by_gid, get_user_by_uid};
extern crate libc;
use std::ffi::CString;

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim();
            let mut parts = input.split_whitespace();
            let command = parts.next().unwrap_or("");
            let args = parts.collect::<Vec<&str>>();

            match command {
                "cd" => {
                    let new_dir = args.first().map_or("/", |x| *x);
                    let root = Path::new(new_dir);
                    if let Err(e) = env::set_current_dir(root) {
                        eprintln!("{}", e);
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
                    list_directory(Path::new("."), long_format, all, classify);
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
                }
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
                }
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

fn parse_flags(args: &[&str]) -> Vec<String> {
    let mut parsed_flags = Vec::new();

    for &arg in args {
        if arg.starts_with('-') && arg.len() > 2 {
            arg.chars()
                .skip(1)
                .for_each(|c| parsed_flags.push(format!("-{}", c)));
        } else {
            parsed_flags.push(arg.to_string());
        }
    }

    parsed_flags
}

fn list_directory_entry(path: &Path, metadata: &Metadata) -> String {
    let file_type_indicator = format_permissions(metadata.mode());
    let num_links = metadata.nlink();
    let owner = get_user_by_uid(metadata.uid())
        .map(|u| u.name().to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.uid().to_string());
    let group = get_group_by_gid(metadata.gid())
        .map(|g| g.name().to_string_lossy().into_owned())
        .unwrap_or_else(|| metadata.gid().to_string());
    let size = metadata.len();

    // Use `timestamp_opt` instead of `timestamp` and handle the result appropriately
    let datetime: DateTime<Local> = match Local.timestamp_opt(metadata.mtime(), 0) {
        chrono::LocalResult::Single(dt) => dt,
        _ => panic!("Invalid timestamp"),
    };
    let datetime_str = datetime.format("%b %e %H:%M").to_string();

    let name = if path.ends_with(".") {
        ".".to_string()
    } else if path.ends_with("..") {
        "..".to_string()
    } else {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };

    format!(
        "{} {:>3} {} {} {:>6} {} {}",
        file_type_indicator, num_links, owner, group, size, datetime_str, name
    )
}
// When printing the total, consider how you want to represent this total in terms of your filesystem's block size.
// The division or adjustment might be needed if you're converting between block sizes or aligning with how `ls` reports its total.
fn list_directory(dir: &Path, long_format: bool, all: bool, _classify: bool) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            all || !entry
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with('.')
        })
        .collect();

    // Custom sort: Ignore leading '.' for hidden files and directories except for '.' and '..'
    entries.sort_by_key(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        match name.as_str() {
            "." | ".." => String::from(""), // Keep these at the top
            _ => name.strip_prefix('.').unwrap_or(&name).to_lowercase(), // Ignore leading dot for sorting
        }
    });

    if long_format {
        let total_blocks = calculate_total_blocks(dir);
        println!("total {}", total_blocks);

        // Manually print '.' and '..' with their metadata
        print_metadata(dir, true); // Current directory '.'
        print_metadata(&dir.join(".."), true); // Parent directory '..'
    }

    // Print remaining entries
    for entry in entries {
        let path = entry.path();
        let metadata = entry.metadata().unwrap(); // Handle errors appropriately
        let display_str = if long_format {
            list_directory_entry(&path, &metadata)
        } else {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };
        println!("{}", display_str);
    }
}

fn print_metadata(path: &Path, long_format: bool) {
    if long_format {
        let metadata = fs::metadata(path).unwrap(); // Handle errors appropriately
        println!("{}", list_directory_entry(path, &metadata));
    }
}

fn get_file_blocks(path: &Path) -> Option<f64> {
    let path_cstr = match path.to_str() {
        Some(path_str) => CString::new(path_str).expect("CString::new failed"),
        None => return None,
    };

    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::stat(path_cstr.as_ptr(), &mut stat) } == 0 {
        let blocks = (stat.st_blocks as f64 + 1.0).floor() / 2.0; // Convert to 1024-byte blocks
        println!("Path {:?}, Blocks: {}", path_cstr, blocks);        
        Some(blocks)
    } else {
        None
    }
}

fn calculate_total_blocks(dir: &Path) -> i64 {
    let mut total_blocks = 0.0;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.metadata().is_ok() {
                if let Some(blocks) = get_file_blocks(&entry.path()) {
                    println!("Blocks for {:?}: {}", entry.path(), blocks);
                    total_blocks += blocks;
                }
            }
        }
    }

    // Optionally, add the block size of the directory itself
    if let Some(blocks) = get_file_blocks(dir) {
        total_blocks += blocks;
    }

    total_blocks.ceil() as i64
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
        destination.join(
            source
                .file_name()
                .ok_or_else(|| "Invalid file name".to_string())?,
        )
    } else {
        destination.to_path_buf()
    };

    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn move_item(source: &Path, destination: &Path) -> Result<(), String> {
    let destination = if destination.is_dir() {
        destination.join(
            source
                .file_name()
                .ok_or_else(|| "Invalid file name".to_string())?,
        )
    } else {
        destination.to_path_buf()
    };

    fs::rename(source, destination).map_err(|e| e.to_string())
}

fn format_permissions(mode: u32) -> String {
    let mut perms = String::with_capacity(10);
    let types = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];

    perms.push(if mode & 0o40000 == 0o40000 {
        'd'
    } else if mode & 0o100000 == 0o100000 {
        '-'
    } else {
        '?'
    });
    perms.push_str(types[((mode >> 6) & 7) as usize]);
    perms.push_str(types[((mode >> 3) & 7) as usize]);
    perms.push_str(types[(mode & 7) as usize]);

    perms
}
