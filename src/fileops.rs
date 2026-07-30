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
