use chrono::TimeZone;
use chrono::Local;
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
use std::os::unix::fs::FileTypeExt;
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
                                        list_directory_entry(path, &metadata, classify, long_format)
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
    long_format: bool,
) -> String {
    let file_type_indicator = format_permissions(metadata.mode() as mode_t);
    let num_links = metadata.nlink();
    let owner = get_user_name_by_uid(metadata.uid()).unwrap_or_else(|| metadata.uid().to_string());
    let group = get_group_name_by_gid(metadata.gid()).unwrap_or_else(|| metadata.gid().to_string());
    let size = metadata.len();

    // Use `timestamp_opt` instead of `timestamp` and handle the result appropriately
    let datetime_str = match Local.timestamp_opt(metadata.mtime(), 0) {
        chrono::LocalResult::Single(dt) => dt.format("%b %e %H:%M").to_string(),
        _ => "??? ?? ??:??".to_string(),
    };

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

    if long_format {
        format!(
            "{} {:>3} {} {} {:>6} {} {}{}",
            file_type_indicator, num_links, owner, group, size, datetime_str, name, classification_char
        )
    } else {
        format!("{}{}", name, classification_char)
    }
}

fn get_file_classification_char(metadata: &Metadata) -> String {
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        "/".to_string()
    } else if file_type.is_symlink() {
        "@".to_string()
    } else if file_type.is_fifo() {
        "|".to_string()
    } else if file_type.is_socket() {
        "=".to_string()
    } else if metadata.permissions().mode() & 0o111 != 0 {
        "*".to_string()
    } else {
        "".to_string()
    }
}

// When printing the total, consider how you want to represent this total in terms of your filesystem's block size.
// The division or adjustment might be needed if you're converting between block sizes or aligning with how `ls` reports its total.
fn list_directory(dir: &Path, long_format: bool, all: bool, classify: bool) {
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(e) => {
            eprintln!("ls: cannot access '{}': {}", dir.display(), e);
            return;
        }
    };

    let mut entries: Vec<_> = read_dir
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
        print_metadata(dir, true, classify); // Current directory '.'
        print_metadata(&dir.join(".."), true, classify); // Parent directory '..'
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
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(e) => {
                eprintln!("ls: cannot access '{}': {}", path.display(), e);
                continue;
            }
        };
        let display_str = list_directory_entry(&path, &metadata, classify, long_format);

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

fn print_metadata(path: &Path, long_format: bool, classify: bool) {
    if long_format {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(e) => {
                eprintln!("ls: cannot access '{}': {}", path.display(), e);
                return;
            }
        };
        println!(
            "{}",
            list_directory_entry(path, &metadata, classify, long_format)
        );
    }
}

fn calculate_total_blocks(dir: &Path, all: bool) -> u64 {
    let mut total_blocks = 0.0;

    // `ls` reports totals in 1024-byte blocks; `st_blocks` (from stat) is in 512-byte units.
    let ls_block_size = 1024.0;

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("ls: cannot access '{}': {}", dir.display(), e);
            return 0;
        }
    };

    for entry in entries.flatten() {
        // Convert the filename part of the path to a string slice if possible
        if let Some(filename) = entry.path().file_name().and_then(|n| n.to_str()) {
            // Check if the filename starts with a dot, excluding such files
            if filename.starts_with('.') && !all {
                // println!("Skipping hidden file: {:?}", entry.path());
                continue;
            }
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(e) => {
                eprintln!("ls: cannot access '{}': {}", entry.path().display(), e);
                continue;
            }
        };
        total_blocks += metadata.blocks() as f64 * 512.0 / ls_block_size;
    }

    // Accurately calculate blocks for "." and ".."
    let dot_blocks = calculate_dir_blocks(dir, ls_block_size);
    let dotdot_blocks = calculate_dir_blocks(&dir.join(".."), ls_block_size);
    if all {
        total_blocks += dot_blocks + dotdot_blocks;
    }

    // Perform ceiling operation on the total blocks to round up to the nearest integer
    total_blocks.ceil() as u64
}

fn calculate_dir_blocks(dir: &Path, ls_block_size: f64) -> f64 {
    fs::metadata(dir)
        .map(|metadata| metadata.blocks() as f64 * 512.0 / ls_block_size)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("zero_shell_test_{}_{}", std::process::id(), name))
    }

    fn as_str_vec(v: &[String]) -> Vec<&str> {
        v.iter().map(|s| s.as_str()).collect()
    }

    #[test]
    fn parse_flags_expands_combined_short_flags() {
        assert_eq!(as_str_vec(&parse_flags(&["-la"])), vec!["-l", "-a"]);
    }

    #[test]
    fn parse_flags_leaves_separate_flags_untouched() {
        assert_eq!(as_str_vec(&parse_flags(&["-l", "-a"])), vec!["-l", "-a"]);
    }

    #[test]
    fn parse_flags_leaves_non_flag_arguments_untouched() {
        assert_eq!(as_str_vec(&parse_flags(&["file.txt"])), vec!["file.txt"]);
    }

    #[test]
    fn parse_flags_handles_mixed_flags_and_paths() {
        assert_eq!(
            as_str_vec(&parse_flags(&["-la", "file.txt"])),
            vec!["-l", "-a", "file.txt"]
        );
    }

    #[test]
    fn format_permissions_regular_file() {
        assert_eq!(format_permissions(0o100644), "-rw-r--r--");
    }

    #[test]
    fn format_permissions_directory() {
        assert_eq!(format_permissions(0o040755), "drwxr-xr-x");
    }

    #[test]
    fn format_permissions_executable() {
        assert_eq!(format_permissions(0o100777), "-rwxrwxrwx");
    }

    #[test]
    fn format_permissions_symlink() {
        assert_eq!(format_permissions(0o120777), "lrwxrwxrwx");
    }

    #[test]
    fn classification_char_for_directory() {
        let path = temp_path("classify_dir");
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "/");
        fs::remove_dir_all(&path).unwrap();
    }

    #[test]
    fn classification_char_for_executable_file() {
        let path = temp_path("classify_exec");
        fs::write(&path, b"").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "*");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn classification_char_for_plain_file() {
        let path = temp_path("classify_plain");
        fs::write(&path, b"").unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn classification_char_for_symlink() {
        let target = temp_path("classify_symlink_target");
        let link = temp_path("classify_symlink_link");
        fs::write(&target, b"").unwrap();
        let _ = fs::remove_file(&link);
        symlink(&target, &link).unwrap();
        let metadata = fs::symlink_metadata(&link).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "@");
        fs::remove_file(&link).unwrap();
        fs::remove_file(&target).unwrap();
    }

    #[test]
    fn classification_char_for_fifo() {
        let path = temp_path("classify_fifo");
        let _ = fs::remove_file(&path);
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
        assert_eq!(result, 0, "mkfifo failed");
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "|");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn classification_char_for_socket() {
        let path = temp_path("classify_socket");
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(get_file_classification_char(&metadata), "=");
        drop(listener);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn list_directory_entry_short_format_appends_classification_char() {
        let path = temp_path("entry_short_exec");
        fs::write(&path, b"").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let entry = list_directory_entry(&path, &metadata, true, false);
        let expected_name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(entry, format!("{}*", expected_name));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn list_directory_entry_long_format_includes_size_and_name() {
        let path = temp_path("entry_long_plain");
        fs::write(&path, b"hello").unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let entry = list_directory_entry(&path, &metadata, false, true);
        assert!(entry.starts_with("-rw"), "unexpected entry: {}", entry);
        assert!(entry.contains(&format!("{:>6}", metadata.len())));
        assert!(entry.ends_with("entry_long_plain"));
        fs::remove_file(&path).unwrap();
    }
}

