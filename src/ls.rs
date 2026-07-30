use crate::users::{get_group_name_by_gid, get_user_name_by_uid};
use chrono::{Local, TimeZone};
use libc::mode_t;
use std::fs;
use std::fs::Metadata;
use std::io::Write;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub fn list_directory_entry(
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
        String::new()
    };

    if long_format {
        format!(
            "{file_type_indicator} {num_links:>3} {owner} {group} {size:>6} {datetime_str} {name}{classification_char}"
        )
    } else {
        format!("{name}{classification_char}")
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
        String::new()
    }
}

// When printing the total, consider how you want to represent this total in terms of your filesystem's block size.
// The division or adjustment might be needed if you're converting between block sizes or aligning with how `ls` reports its total.
pub fn list_directory(
    dir: &Path,
    long_format: bool,
    all: bool,
    classify: bool,
    recursive: bool,
    output: &mut dyn Write,
) {
    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(e) => {
            eprintln!("ls: cannot access '{}': {}", dir.display(), e);
            return;
        }
    };

    let mut entries: Vec<_> = read_dir
        .filter_map(Result::ok)
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
            "." | ".." => String::new(), // Keep these at the top
            _ => name.strip_prefix('.').unwrap_or(&name).to_lowercase(), // Ignore leading dot for sorting
        }
    });

    if long_format && all {
        let total_blocks = calculate_total_blocks(dir, all);
        let _ = writeln!(output, "total {total_blocks}");

        // Manually print '.' and '..' with their metadata
        print_metadata(dir, true, classify, output); // Current directory '.'
        print_metadata(&dir.join(".."), true, classify, output); // Parent directory '..'
    } else if long_format && !all {
        let total_blocks = calculate_total_blocks(dir, all);
        let _ = writeln!(output, "total {total_blocks}");
    }

    if all && !long_format && !classify {
        let _ = write!(output, ".  ");
        let _ = write!(output, "..  ");
    }

    if all && !long_format && classify {
        let _ = write!(output, "./  ");
        let _ = write!(output, "../  ");
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
            let _ = writeln!(output);
        } else if !long_format {
            if entry.path() == entries[length - 1].path() {
                let _ = write!(output, "{display_str}  ");
                let _ = writeln!(output);
            } else {
                let _ = write!(output, "{display_str}  ");
            }
        } else {
            let _ = writeln!(output, "{display_str}");
        }
    }

    if recursive {
        let subdirs: Vec<PathBuf> = entries
            .iter()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();

        for subdir in subdirs {
            let _ = writeln!(output);
            let _ = writeln!(output, "{}:", subdir.display());
            list_directory(&subdir, long_format, all, classify, recursive, output);
        }
    }
}

fn print_metadata(path: &Path, long_format: bool, classify: bool, output: &mut dyn Write) {
    if long_format {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(e) => {
                eprintln!("ls: cannot access '{}': {}", path.display(), e);
                return;
            }
        };
        let _ = writeln!(
            output,
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
    fs::metadata(dir).map_or(0.0, |metadata| {
        metadata.blocks() as f64 * 512.0 / ls_block_size
    })
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
    perms.push_str(types[(mode & 7) as usize]); // Others

    perms
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("zero_shell_test_{}_{}", std::process::id(), name))
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
        assert_eq!(entry, format!("{expected_name}*"));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn list_directory_entry_long_format_includes_size_and_name() {
        let path = temp_path("entry_long_plain");
        fs::write(&path, b"hello").unwrap();
        let metadata = fs::symlink_metadata(&path).unwrap();
        let entry = list_directory_entry(&path, &metadata, false, true);
        assert!(entry.starts_with("-rw"), "unexpected entry: {entry}");
        assert!(entry.contains(&format!("{:>6}", metadata.len())));
        assert!(entry.ends_with("entry_long_plain"));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn list_directory_recursive_descends_into_subdirectories() {
        let dir = temp_path("recursive_root");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/inner.txt"), b"").unwrap();

        let mut output = Vec::new();
        list_directory(&dir, false, false, false, true, &mut output);
        let text = String::from_utf8(output).unwrap();

        assert!(text.contains(&format!("{}:", dir.join("sub").display())));
        assert!(text.contains("inner.txt"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn list_directory_non_recursive_does_not_descend() {
        let dir = temp_path("non_recursive_root");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/inner.txt"), b"").unwrap();

        let mut output = Vec::new();
        list_directory(&dir, false, false, false, false, &mut output);
        let text = String::from_utf8(output).unwrap();

        assert!(!text.contains("inner.txt"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
