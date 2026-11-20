//! Reversible AGY host integration shared by the CLI and the TUI.

use crate::cc_settings::Settings;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Installation {
    pub settings: PathBuf,
    pub home: PathBuf,
}

/// This record intentionally contains no unrelated host settings.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Record {
    schema: u32,
    settings: PathBuf,
    executable: PathBuf,
    installed_at: String,
    active: bool,
    original_present: bool,
    original_status_line: Value,
    installed_status_line: Value,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub state: &'static str,
    pub settings: PathBuf,
    pub executable: PathBuf,
    pub record: PathBuf,
    pub installed_at: Option<String>,
}

impl std::fmt::Display for Status {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(formatter, "agyline: {}", self.state)?;
        writeln!(formatter, "Settings: {}", self.settings.display())?;
        writeln!(formatter, "Executable: {}", self.executable.display())?;
        write!(formatter, "Record: {}", self.record.display())?;
        if let Some(installed_at) = &self.installed_at {
            write!(formatter, "\nInstalled at: {installed_at}")?;
        }
        Ok(())
    }
}

impl Installation {
    pub fn new(settings: PathBuf, home: PathBuf) -> Self {
        Self { settings, home }
    }

    pub fn user() -> Self {
        Self::new(Settings::user_path(), crate::paths::home())
    }

    pub fn record_path(&self) -> PathBuf {
        self.home.join("installation.json")
    }

    pub fn executable_path(&self) -> PathBuf {
        self.home.join(if cfg!(windows) {
            "agyline.exe"
        } else {
            "agyline"
        })
    }

    pub fn command(&self) -> Result<String, String> {
        command_for_path(&absolute(&self.executable_path())?)
    }

    fn read_record(&self) -> Result<Option<Record>, String> {
        let text = match fs::read_to_string(self.record_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let record: Record = serde_json::from_str(&text)
            .map_err(|error| format!("invalid installation record: {error}"))?;
        if record.schema != 1
            || record.settings != absolute(&self.settings)?
            || record.executable != absolute(&self.executable_path())?
            || !record.installed_status_line.is_object()
            || (record.original_present && !record.original_status_line.is_object())
        {
            return Err(
                "installation record belongs to different paths or has an unsupported schema"
                    .into(),
            );
        }
        Ok(Some(record))
    }

    fn write_record(&self, record: &Record) -> Result<(), String> {
        let mut text = serde_json::to_string_pretty(record).map_err(|error| error.to_string())?;
        text.push('\n');
        crate::paths::write_atomic(&self.record_path(), &text).map_err(|error| error.to_string())
    }

    /// Previewing does not create directories, backups or records.
    pub fn preview_install(&self) -> Result<(Settings, Settings), String> {
        let base = Settings::load(&self.settings)?;
        if let Some(record) = self.read_record()?.filter(|record| record.active) {
            ensure_recoverable(&base, &record)?;
        }
        let mut draft = base.clone();
        let mut line = base.status_line().unwrap_or_default();
        line.command = self.command()?;
        line.enabled = Some(true);
        line.stack_with_default = line.stack_with_default.or(Some(false));
        draft.set_status_line(Some(&line));
        Ok((base, draft))
    }

    pub fn install(&self) -> Result<Status, String> {
        let (base, draft) = self.preview_install()?;
        self.apply_install(&base, &draft)
    }

    /// Apply exactly the reviewed status line, detecting edits since preview.
    pub fn apply_install(&self, base: &Settings, draft: &Settings) -> Result<Status, String> {
        let _locks = self.lock()?;
        if base.path != self.settings || draft.path != self.settings || base.stale() {
            return Err("settings.json changed on disk; reload and review again".into());
        }
        if draft
            .changed_keys(base)
            .iter()
            .any(|key| key != "statusLine")
        {
            return Err("integration may only change statusLine".into());
        }
        let installed = draft
            .status_line_value()
            .cloned()
            .ok_or("missing statusLine")?;
        crate::cc_settings::validate_status_line(Some(&installed))?;
        let line = draft.status_line().ok_or("missing statusLine command")?;
        if line.command != self.command()? {
            return Err("installation command must use the stable agyline executable".into());
        }
        let existing = self.read_record()?;
        let active = existing.as_ref().filter(|record| record.active).cloned();
        if let Some(record) = &active {
            ensure_recoverable(base, record)?;
        }
        copy_executable(
            &std::env::current_exe().map_err(|error| error.to_string())?,
            &self.executable_path(),
        )?;
        if active.as_ref().is_some_and(|record| {
            record.installed_status_line == installed
                && base.status_line_value() == Some(&record.installed_status_line)
        }) {
            return self.status();
        }
        let mut record = active.unwrap_or(Record {
            schema: 1,
            settings: absolute(&self.settings)?,
            executable: absolute(&self.executable_path())?,
            installed_at: chrono::Utc::now().to_rfc3339(),
            active: true,
            original_present: base.status_line_value().is_some(),
            original_status_line: base.status_line_value().cloned().unwrap_or(Value::Null),
            installed_status_line: installed.clone(),
        });
        record.installed_status_line = installed;
        // Persist the recovery point before settings. If interrupted here,
        // an exact original-value match allows a later explicit retry.
        self.write_record(&record)?;
        if let Err(error) = draft.save(base) {
            // No settings commit occurred. Restore the previous record so a
            // permission error or concurrent editor does not poison retries.
            let rollback = match &existing {
                Some(previous) => self.write_record(previous),
                None => fs::remove_file(self.record_path()).map_err(|error| error.to_string()),
            };
            return Err(match rollback {
                Ok(()) => error.to_string(),
                Err(rollback_error) => {
                    format!("{error}; could not roll back installation record: {rollback_error}")
                }
            });
        }
        self.status()
    }

    pub fn preview_uninstall(&self) -> Result<(Settings, Settings), String> {
        let base = Settings::load(&self.settings)?;
        let mut draft = base.clone();
        if let Some(record) = self.read_record()?.filter(|record| record.active) {
            ensure_recoverable(&base, &record)?;
            draft.set_status_line_value(
                record
                    .original_present
                    .then_some(record.original_status_line),
            );
        }
        Ok((base, draft))
    }

    pub fn uninstall(&self) -> Result<Status, String> {
        let (base, _) = self.preview_uninstall()?;
        self.apply_uninstall(&base)
    }

    pub fn apply_uninstall(&self, reviewed: &Settings) -> Result<Status, String> {
        let _locks = self.lock()?;
        if reviewed.path != self.settings || reviewed.stale() {
            return Err("settings.json changed on disk; reload and review again".into());
        }
        let Some(mut record) = self.read_record()?.filter(|record| record.active) else {
            return self.status();
        };
        ensure_recoverable(reviewed, &record)?;
        if !matches_original_status_line(reviewed, &record) {
            let mut restored = reviewed.clone();
            restored.set_status_line_value(
                record
                    .original_present
                    .then_some(record.original_status_line.clone()),
            );
            restored.save(reviewed).map_err(|error| error.to_string())?;
        }
        // A previous uninstall may have committed settings and then failed
        // to update this record. Do not rewrite settings in that case.
        record.active = false;
        self.write_record(&record)?;
        // Keep the binary: it may be this running executable on Windows.
        self.status()
    }

    pub fn status(&self) -> Result<Status, String> {
        let settings = Settings::load(&self.settings)?;
        let record = self.read_record()?;
        let state = match &record {
            Some(record) if record.active => {
                if settings.status_line_value() != Some(&record.installed_status_line) {
                    if matches_original_status_line(&settings, record) {
                        "recovery pending: statusLine matches original; retry install or uninstall"
                    } else {
                        "conflict: statusLine changed since installation"
                    }
                } else if !self.executable_path().is_file() {
                    "broken: installed executable missing"
                } else {
                    "installed"
                }
            }
            Some(_) => "uninstalled (restoration record retained)",
            None => "not installed (no restoration record)",
        };
        Ok(Status {
            state,
            settings: absolute(&self.settings)?,
            executable: absolute(&self.executable_path())?,
            record: absolute(&self.record_path())?,
            installed_at: record.map(|record| record.installed_at),
        })
    }

    fn lock(&self) -> Result<(Lock, Lock), String> {
        fs::create_dir_all(&self.home).map_err(|error| error.to_string())?;
        if let Some(parent) = self.settings.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let home_lock = Lock::acquire(self.home.join("installation.lock"))?;
        // Use the resolved host path, so two different tool homes and symlink
        // aliases cannot concurrently modify the same host through this API.
        let target = fs::canonicalize(&self.settings).unwrap_or(absolute(&self.settings)?);
        let mut lock_name = target.as_os_str().to_os_string();
        lock_name.push(".agyline-lock");
        let settings_lock = Lock::acquire(PathBuf::from(lock_name))?;
        Ok((home_lock, settings_lock))
    }
}

fn matches_original_status_line(settings: &Settings, record: &Record) -> bool {
    let original = record
        .original_present
        .then_some(&record.original_status_line);
    settings.status_line_value() == original
}

fn ensure_recoverable(settings: &Settings, record: &Record) -> Result<(), String> {
    if settings.status_line_value() != Some(&record.installed_status_line)
        && !matches_original_status_line(settings, record)
    {
        return Err("statusLine changed since installation; refusing to overwrite it (see installation.json)".into());
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| error.to_string())
}

/// Host commands execute through cmd.exe on Windows and sh on Unix.
pub fn command_for_path(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or("executable path is not valid Unicode")?;
    if path.chars().any(char::is_control) {
        return Err("executable path contains control characters".into());
    }
    if cfg!(windows) {
        if path.contains(['"', '%', '!', '^', '&', '|', '<', '>', '(', ')']) {
            return Err(
                "Windows executable path contains unsupported shell expansion or command symbols"
                    .into(),
            );
        }
        Ok(format!("\"{path}\""))
    } else {
        Ok(format!("'{}'", path.replace('\'', "'\"'\"'")))
    }
}

fn copy_executable(source: &Path, destination: &Path) -> Result<(), String> {
    let source_identity = fs::canonicalize(source).map_err(|error| error.to_string())?;
    if fs::canonicalize(destination).ok().as_ref() == Some(&source_identity) {
        return Ok(());
    }
    if fs::read(source)
        .ok()
        .zip(fs::read(destination).ok())
        .is_some_and(|(source, destination)| source == destination)
    {
        return Ok(());
    }
    let temporary = destination.with_extension(format!("copy-{}", std::process::id()));
    let mut staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("cannot exclusively create installation staging file: {error}"))?;
    let staged_result = (|| -> std::io::Result<()> {
        let mut source_file = fs::File::open(source)?;
        std::io::copy(&mut source_file, &mut staged)?;
        staged.set_permissions(source_file.metadata()?.permissions())?;
        staged.sync_all()
    })();
    drop(staged);
    staged_result
        .and_then(|()| fs::rename(&temporary, destination))
        .map_err(|error| {
            let _ = fs::remove_file(&temporary);
            format!("cannot replace stable executable (it may still be running): {error}")
        })
}

struct Lock {
    // Closing the handle releases the OS lock, including on process exit.
    _file: fs::File,
}

impl Lock {
    fn acquire(path: PathBuf) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| {
                format!(
                    "cannot acquire {} (another operation may be running): {error}",
                    path.display()
                )
            })?;
        file.try_lock().map_err(|error| {
            format!(
                "cannot acquire {} (another operation may be running): {error}",
                path.display()
            )
        })?;
        // Keep the file itself: unlinking it could let another process lock
        // a replacement inode while this handle still owns the old lock.
        Ok(Self { _file: file })
    }
}
