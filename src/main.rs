use chrono::TimeZone;
use chrono::{DateTime, Local};
use std::env;
use std::fs;
use std::fs::Metadata;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::exit;
extern crate libc;
use libc::{getgrgid_r, getpwuid_r, group, passwd};
use std::ffi::CStr;
use std::mem;
use std::ptr;
use std::os::unix::fs::PermissionsExt;
use libc::mode_t;


// Function to get username by UID
fn get_user_name_by_uid(uid: u32) -> Option<String> {
    let mut pwd = unsafe { mem::zeroed() };
    let mut buf = vec![0u8; 1024];
    let mut result = ptr::null_mut();
    unsafe {
        if getpwuid_r(uid, &mut pwd, buf.as_mut_ptr() as *mut _, buf.len(), &mut result) == 0
            && !result.is_null() {
            return Some(CStr::from_ptr(pwd.pw_name).to_string_lossy().into_owned());
        }
    }
    None
}

// Function to get group name by GID
fn get_group_name_by_gid(gid: u32) -> Option<String> {
    let mut grp = unsafe { mem::zeroed() };
    let mut buf = vec![0u8; 1024];
    let mut result = ptr::null_mut();
    unsafe {
        if getgrgid_r(gid, &mut grp, buf.as_mut_ptr() as *mut _, buf.len(), &mut result) == 0
            && !result.is_null() {
            return Some(CStr::from_ptr(grp.gr_name).to_string_lossy().into_owned());
        }
    }
    None
}

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

fn list_directory_entry(
    path: &Path,
    metadata: &Metadata,
    classify: bool,
    all: bool,
    long_format: bool,
) -> String {
    let file_type_indicator = format_permissions(metadata.mode() as mode_t);
    let num_links = metadata.nlink();
    let owner = get_user_name_by_uid(metadata.uid()).unwrap_or_else(|| metadata.uid().to_string());
    let group = get_group_name_by_gid(metadata.gid()).unwrap_or_else(|| metadata.gid().to_string());
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

    let classification_char = if classify {
        get_file_classification_char(metadata)
    } else {
        "".to_string()
    };

    if classify && !all && !long_format {
        format!("{}{}", name, classification_char)
    } else if all && classify && !long_format {
        format!("{}{}", name, classification_char)
    } else if all && !classify && !long_format {
        format!("{}", name)
    } else if long_format && all && !classify {
        format!(
            "{} {:>3} {} {} {:>6} {} {}",
            file_type_indicator, num_links, owner, group, size, datetime_str, name
        )
    } else if !classify && !all && !long_format {
        format!("{}", name)
    } else if classify && all && long_format {
        // Append the classification_char to the formatted string
        format!(
            "{} {:>3} {} {} {:>6} {} {}{}",
            file_type_indicator,
            num_links,
            owner,
            group,
            size,
            datetime_str,
            name,
            classification_char
        )
    } else {
        format!(
            "{} {:>3} {} {} {:>6} {} {}",
            file_type_indicator,
            num_links,
            owner,
            group,
            size,
            datetime_str,
            name,
        )
    }
}

fn get_file_classification_char(metadata: &Metadata) -> String {
    if metadata.is_dir() {
        "/".to_string()
    } else if metadata.permissions().mode() & 0o111 != 0 {
        "*".to_string()
    } else {
        "".to_string()
    }
}

// When printing the total, consider how you want to represent this total in terms of your filesystem's block size.
// The division or adjustment might be needed if you're converting between block sizes or aligning with how `ls` reports its total.
fn list_directory(dir: &Path, long_format: bool, all: bool, classify: bool) {
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

    if long_format && all {
        let total_blocks = calculate_total_blocks(dir, all);
        println!("total {}", total_blocks);

        // Manually print '.' and '..' with their metadata
        print_metadata(dir, true, classify, all); // Current directory '.'
        print_metadata(&dir.join(".."), true, classify, all); // Parent directory '..'
    } else if long_format && !all {
        let total_blocks = calculate_total_blocks(dir, all);
        println!("total {}", total_blocks);
    }

    if all && !long_format && !classify {
        print!(".  ");
        print!("..  ");
    }

    if all && !long_format && classify {
        print!("./  ");
        print!("../  ");
    }

    // Print remaining entries
    for entry in &entries {
        // println!("Entries: {:?}", entries);
        let length = entries.len();
        let path = entry.path();
        let metadata = entry.metadata().unwrap(); // Handle errors appropriately
        let display_str = list_directory_entry(&path, &metadata, classify, all, long_format);

        if length == 0 {
            println!();
        } else
        if !long_format {

            if entry.path() != entries[length - 1].path() {
                print!("{}  ", display_str);
            } else {
                print!("{}  ", display_str);
                println!()
            }
        } else {
            println!("{}", display_str);
        }
    }
}

fn print_metadata(path: &Path, long_format: bool, classify: bool, all: bool) {
    if long_format {
        let metadata = fs::metadata(path).unwrap(); // Handle errors appropriately
        println!(
            "{}",
            list_directory_entry(path, &metadata, classify, all, long_format)
        );
    }
}

fn calculate_total_blocks(dir: &Path, all: bool) -> u64 {
    let mut total_blocks = 0.0;

    // Assuming physical_block_size and ls_block_size are constants for all files in this context
    let physical_block_size = 4096.0; // Common filesystem block size in bytes
    let ls_block_size = 1024.0; // Block size used by `ls` in bytes

    let entries =
        fs::read_dir(dir).unwrap_or_else(|_| panic!("Failed to read directory: {:?}", dir));

    for entry in entries.flatten() {
        // Convert the filename part of the path to a string slice if possible
        if let Some(filename) = entry.path().file_name().and_then(|n| n.to_str()) {
            // Check if the filename starts with a dot, excluding such files
            if filename.starts_with('.') && !all {
                // println!("Skipping hidden file: {:?}", entry.path());
                continue;
            }
        }

        let metadata = entry
            .metadata()
            .unwrap_or_else(|_| panic!("Failed to get metadata for entry: {:?}", entry.path()));
        let file_physical_blocks_in_use = metadata.blocks() as f64; // st_blocks reports 512-byte blocks
        let blocks_used = (file_physical_blocks_in_use * 512.0 / physical_block_size)
            * (physical_block_size / ls_block_size);
        total_blocks += blocks_used;
    }

    // Accurately calculate blocks for "." and ".."
    let dot_blocks = calculate_dir_blocks(dir, physical_block_size, ls_block_size);
    let dotdot_blocks = calculate_dir_blocks(&dir.join(".."), physical_block_size, ls_block_size);
    if all {
        total_blocks += dot_blocks + dotdot_blocks;
    }

    // Perform ceiling operation on the total blocks to round up to the nearest integer
    total_blocks.ceil() as u64
}

fn calculate_dir_blocks(dir: &Path, physical_block_size: f64, ls_block_size: f64) -> f64 {
    fs::metadata(dir)
        .map(|metadata| {
            let blocks_in_use = metadata.blocks() as f64; // st_blocks reports 512-byte blocks
            (blocks_in_use * 512.0 / physical_block_size) * (physical_block_size / ls_block_size)
        })
        .unwrap_or(0.0)
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

fn format_permissions(mode: mode_t) -> String {
    let mut perms = String::with_capacity(10);

    // Determine file type
    perms.push(match mode & libc::S_IFMT {
        libc::S_IFDIR => 'd',
        libc::S_IFCHR => 'c',
        libc::S_IFBLK => 'b',
        libc::S_IFREG => '-',
        libc::S_IFLNK => 'l',
        libc::S_IFSOCK => 's',
        libc::S_IFIFO => 'p',
        _ => '?',
    });

    // Determine permissions (owner, group, others)
    let types = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
    perms.push_str(types[((mode >> 6) & 7) as usize]); // Owner
    perms.push_str(types[((mode >> 3) & 7) as usize]); // Group
    perms.push_str(types[(mode & 7) as usize]);        // Others

    perms
}

