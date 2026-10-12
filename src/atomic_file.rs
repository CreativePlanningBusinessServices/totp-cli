use std::io::Write;
use std::path::Path;

/// Writes `contents` to `path` so that readers see either the old file or the complete new
/// one, never a truncated mix: write a sibling `.tmp`, then rename it over the target. On
/// Unix the temp file is created with mode 0600 so the contents are never world-readable,
/// not even briefly. Errors come back as plain messages so each caller can wrap them in
/// its own `CliError` variant.
pub fn write_private(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|io_error| format!("cannot create {}: {io_error}", parent.display()))?;

    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp_path = parent.join(format!("{file_name}.tmp"));
    let mut temp_file = open_private(&temp_path)
        .map_err(|io_error| format!("cannot write {}: {io_error}", temp_path.display()))?;
    temp_file
        .write_all(contents.as_bytes())
        .and_then(|()| temp_file.sync_all())
        .map_err(|io_error| format!("cannot write {}: {io_error}", temp_path.display()))?;
    drop(temp_file);

    std::fs::rename(&temp_path, path).map_err(|io_error| {
        let _ = std::fs::remove_file(&temp_path);
        format!("cannot replace {}: {io_error}", path.display())
    })
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::create(path)
}
