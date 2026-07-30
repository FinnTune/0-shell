use std::fs;
use std::path::Path;

pub fn remove_item(path: &Path, recursive: bool) -> Result<(), String> {
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

pub fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
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

pub fn move_item(source: &Path, destination: &Path) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zero_shell_fileops_test_{}_{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn remove_item_deletes_a_file() {
        let dir = temp_dir("remove_file");
        let file = dir.join("a.txt");
        fs::write(&file, b"hi").unwrap();

        assert!(remove_item(&file, false).is_ok());
        assert!(!file.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn remove_item_non_recursive_errors_on_directory() {
        let dir = temp_dir("remove_dir_no_r");

        assert!(remove_item(&dir, false).is_err());
        assert!(dir.exists(), "directory should be left untouched");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn remove_item_recursive_deletes_nested_contents() {
        let dir = temp_dir("remove_recursive");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), b"hi").unwrap();
        fs::write(dir.join("sub/b.txt"), b"hi").unwrap();

        assert!(remove_item(&dir, true).is_ok());
        assert!(!dir.exists());
    }

    #[test]
    fn remove_item_errors_on_missing_file() {
        let dir = temp_dir("remove_missing");
        let missing = dir.join("nope.txt");

        assert!(remove_item(&missing, false).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copy_file_to_exact_destination_path() {
        let dir = temp_dir("copy_exact");
        let source = dir.join("a.txt");
        let destination = dir.join("b.txt");
        fs::write(&source, b"hello").unwrap();

        assert!(copy_file(&source, &destination).is_ok());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "hello");
        assert!(source.exists(), "copy should not remove the source");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copy_file_into_directory_uses_source_filename() {
        let dir = temp_dir("copy_into_dir");
        let source = dir.join("a.txt");
        let destination_dir = dir.join("dest");
        fs::write(&source, b"hello").unwrap();
        fs::create_dir_all(&destination_dir).unwrap();

        assert!(copy_file(&source, &destination_dir).is_ok());
        assert_eq!(
            fs::read_to_string(destination_dir.join("a.txt")).unwrap(),
            "hello"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copy_file_refuses_to_copy_a_directory() {
        let dir = temp_dir("copy_dir_source");
        let source_dir = dir.join("subdir");
        fs::create_dir_all(&source_dir).unwrap();
        let destination = dir.join("dest");

        let result = copy_file(&source_dir, &destination);
        assert!(result.is_err());
        assert!(!destination.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn move_item_to_exact_destination_path() {
        let dir = temp_dir("move_exact");
        let source = dir.join("a.txt");
        let destination = dir.join("b.txt");
        fs::write(&source, b"hello").unwrap();

        assert!(move_item(&source, &destination).is_ok());
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "hello");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn move_item_into_directory_uses_source_filename() {
        let dir = temp_dir("move_into_dir");
        let source = dir.join("a.txt");
        let destination_dir = dir.join("dest");
        fs::write(&source, b"hello").unwrap();
        fs::create_dir_all(&destination_dir).unwrap();

        assert!(move_item(&source, &destination_dir).is_ok());
        assert!(!source.exists());
        assert_eq!(
            fs::read_to_string(destination_dir.join("a.txt")).unwrap(),
            "hello"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn move_item_moves_a_directory() {
        let dir = temp_dir("move_dir");
        let source_dir = dir.join("subdir");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("a.txt"), b"hi").unwrap();
        let destination_dir = dir.join("moved");

        assert!(move_item(&source_dir, &destination_dir).is_ok());
        assert!(!source_dir.exists());
        assert_eq!(
            fs::read_to_string(destination_dir.join("a.txt")).unwrap(),
            "hi"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
