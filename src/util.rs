use std::path::Path;

/// Replace the user's home directory with `~` for display.
pub fn display_path(path: impl AsRef<Path>) -> String {
    if let Ok(home) = std::env::var("HOME") {
        let home = Path::new(&home);
        if let Ok(rest) = path.as_ref().strip_prefix(home) {
            return format!("~/{}", rest.display());
        }
    }
    path.as_ref().display().to_string()
}

/// Calculate the total size of a directory recursively
pub fn dir_size(path: impl AsRef<Path>) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path.as_ref()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total += dir_size(&path);
            }
        }
    }
    total
}

/// Format bytes into a human-readable string
pub fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size > 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1} {}", size, UNITS[unit_idx])
}
