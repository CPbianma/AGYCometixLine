use std::path::PathBuf;

/// agyline's data directory, independent of the host's config override.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGYLINE_HOME").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".gemini")
        .join("antigravity-cli")
        .join("agyline")
}

pub fn config_file() -> PathBuf {
    home().join("config.toml")
}

pub fn models_file() -> PathBuf {
    home().join("models.toml")
}

/// The user's own presets, one file each.
pub fn presets_dir() -> PathBuf {
    home().join("presets")
}

pub fn state_dir() -> PathBuf {
    home().join("state")
}

/// Antigravity CLI's own config directory, honoring `AGY_CONFIG_DIR`.
pub fn agy_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGY_CONFIG_DIR").filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join(".gemini")
        .join("antigravity-cli")
}

/// Internal compatibility name for existing upstream components.
/// This always returns the AGY root; it never consults Claude configuration.
pub fn claude_dir() -> PathBuf {
    agy_dir()
}

/// Write `contents` to `path` atomically: write a sibling temp file, then
/// rename it over the target.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    // Exclusive creation refuses pre-existing symlinks and hard links rather
    // than following them and truncating a file unrelated to this write.
    let mut staged = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let staged_result = staged
        .write_all(contents.as_bytes())
        .and_then(|()| staged.sync_all());
    drop(staged);
    staged_result
        .and_then(|()| std::fs::rename(&tmp, path))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
}
