//! Antigravity CLI user settings (`settings.json`): reading the status line
//! settings, and changing them with a diff first, a backup and an atomic
//! write. Keys keep their order and settings agyline doesn't know are left
//! as they are.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The supported Antigravity CLI `statusLine` fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusLine {
    pub command: String,
    pub padding: Option<u64>,
    pub enabled: Option<bool>,
    pub stack_with_default: Option<bool>,
}

#[derive(Debug)]
pub enum SaveError {
    /// The file changed on disk since it was read.
    Changed,
    Io(std::io::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Changed => write!(f, "settings.json changed on disk; reload and try again"),
            SaveError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// One line of a diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Same,
    Added,
    Removed,
}

/// The settings file and its contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub path: PathBuf,
    /// The file as read; `None` when there was none.
    text: Option<String>,
    /// Always an object.
    value: Value,
}

/// Only these host fields may be generated. Unknown existing keys survive.
const STATUS_KEYS: [&str; 5] = [
    "type",
    "command",
    "padding",
    "enabled",
    "stack_with_default",
];

/// Internal compatibility helper; only recognizes agyline commands.
pub fn is_ccline(command: &str) -> bool {
    command.contains("agyline")
}

/// Stable installation command, never a build-directory executable.
pub fn suggested_command() -> String {
    crate::integration::Installation::user()
        .command()
        .unwrap_or_else(|_| "agyline".into())
}

impl Settings {
    /// `settings.json` in the AGY config directory.
    pub fn user_path() -> PathBuf {
        crate::paths::agy_dir().join("settings.json")
    }

    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let value = match text.as_deref().map(str::trim) {
            None => Value::Object(Map::new()),
            Some(t) => match serde_json::from_str::<Value>(t) {
                Ok(v @ Value::Object(_)) => v,
                Ok(_) => return Err("settings.json isn't a JSON object".into()),
                Err(e) => return Err(format!("settings.json isn't plain JSON: {e}")),
            },
        };
        validate_status_line(value.get("statusLine"))?;
        Ok(Settings {
            path: path.to_path_buf(),
            text,
            value,
        })
    }

    /// Whether there was a file to read.
    pub fn exists(&self) -> bool {
        self.text.is_some()
    }

    /// Whether the file changed on disk since it was read.
    pub fn stale(&self) -> bool {
        std::fs::read_to_string(&self.path).ok() != self.text
    }

    /// Whether this differs from `other` in any setting.
    pub fn differs(&self, other: &Settings) -> bool {
        self.value != other.value
    }

    fn object(&self) -> &Map<String, Value> {
        self.value.as_object().expect("settings are an object")
    }

    fn object_mut(&mut self) -> &mut Map<String, Value> {
        self.value.as_object_mut().expect("settings are an object")
    }

    pub fn status_line(&self) -> Option<StatusLine> {
        let s = self.object().get("statusLine")?;
        Some(StatusLine {
            command: s.get("command")?.as_str()?.to_string(),
            padding: s.get("padding").and_then(Value::as_u64),
            enabled: s.get("enabled").and_then(Value::as_bool),
            stack_with_default: s.get("stack_with_default").and_then(Value::as_bool),
        })
    }

    /// The optional host accessibility preference.
    pub fn prefers_reduced_motion(&self) -> bool {
        self.object()
            .get("prefersReducedMotion")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Exact status line value for narrow, lossless installation recovery.
    pub fn status_line_value(&self) -> Option<&Value> {
        self.object().get("statusLine")
    }

    pub fn set_status_line_value(&mut self, value: Option<Value>) {
        match value {
            Some(value) => {
                self.object_mut().insert("statusLine".into(), value);
            }
            None => {
                self.object_mut().remove("statusLine");
            }
        }
    }

    /// Replace `statusLine`, keeping keys agyline doesn't know.
    pub fn set_status_line(&mut self, line: Option<&StatusLine>) {
        let Some(line) = line else {
            self.object_mut().remove("statusLine");
            return;
        };
        let mut obj = match self.object().get("statusLine") {
            Some(Value::Object(o)) => o.clone(),
            _ => Map::new(),
        };
        let values = [
            Some(Value::from("command")),
            Some(Value::from(line.command.clone())),
            line.padding.map(Value::from),
            line.enabled.map(Value::from),
            line.stack_with_default.map(Value::from),
        ];
        for (key, value) in STATUS_KEYS.iter().zip(values) {
            match value {
                Some(v) => {
                    obj.insert(key.to_string(), v);
                }
                None => {
                    obj.remove(*key);
                }
            }
        }
        self.object_mut()
            .insert("statusLine".into(), Value::Object(obj));
    }

    /// Two-space JSON, preserving the order of existing object keys.
    pub fn render(&self) -> String {
        let mut s = serde_json::to_string_pretty(&self.value).unwrap_or_default();
        s.push('\n');
        s
    }

    /// `"key": value` lines for one top-level key; none when it's absent.
    fn excerpt(&self, key: &str) -> Vec<String> {
        let Some(v) = self.object().get(key) else {
            return Vec::new();
        };
        let pretty = serde_json::to_string_pretty(v).unwrap_or_default();
        pretty
            .lines()
            .enumerate()
            .map(|(i, l)| {
                if i == 0 {
                    format!("{}: {l}", Value::from(key))
                } else {
                    l.to_string()
                }
            })
            .collect()
    }

    /// The top-level keys whose values differ from `before`.
    pub fn changed_keys(&self, before: &Settings) -> Vec<String> {
        let (a, b) = (before.object(), self.object());
        let mut keys: Vec<String> = Vec::new();
        for k in b.keys().chain(a.keys()) {
            if a.get(k) != b.get(k) && !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }

    /// The changed keys, before and after, line by line.
    pub fn diff(&self, before: &Settings) -> Vec<(Mark, String)> {
        let mut out = Vec::new();
        for key in self.changed_keys(before) {
            if !out.is_empty() {
                out.push((Mark::Same, String::new()));
            }
            out.extend(diff_lines(&before.excerpt(&key), &self.excerpt(&key)));
        }
        out
    }

    fn backup_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(
            ".agyline-bak-{}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            std::process::id()
        ));
        self.path.with_file_name(name)
    }

    /// Write the settings unless the file changed since `base` was read,
    /// keeping the old file as a unique `settings.json.agyline-bak-*`. A symlinked file
    /// is written through the link. Returns the backup's path, if any.
    pub fn save(&self, base: &Settings) -> Result<Option<PathBuf>, SaveError> {
        validate_status_line(self.status_line_value()).map_err(|error| {
            SaveError::Io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
        })?;
        if self.path != base.path {
            return Err(SaveError::Changed);
        }
        let on_disk = match std::fs::read_to_string(&self.path) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(SaveError::Io(e)),
        };
        if on_disk != base.text {
            return Err(SaveError::Changed);
        }
        let backup = match &base.text {
            Some(text) => {
                let path = self.backup_path();
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(SaveError::Io)?;
                file.write_all(text.as_bytes()).map_err(SaveError::Io)?;
                file.sync_all().map_err(SaveError::Io)?;
                Some(path)
            }
            None => None,
        };
        // The backup may take time; reject edits made while it was written.
        if base.stale() {
            return Err(SaveError::Changed);
        }
        let target = std::fs::canonicalize(&self.path).unwrap_or_else(|_| self.path.clone());
        crate::paths::write_atomic(&target, &self.render()).map_err(SaveError::Io)?;
        Ok(backup)
    }
}

pub(crate) fn validate_status_line(value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = value
        .as_object()
        .ok_or("statusLine must be a JSON object")?;
    for (key, value) in object {
        let valid = match key.as_str() {
            "type" => value.as_str() == Some("command"),
            "command" => value.is_string(),
            "padding" => value.as_u64().is_some(),
            "enabled" | "stack_with_default" => value.is_boolean(),
            _ => true,
        };
        if !valid {
            return Err(format!("statusLine.{key} has an invalid type"));
        }
    }
    Ok(())
}

/// A line diff of `a` into `b` (longest common subsequence).
pub fn diff_lines(a: &[String], b: &[String]) -> Vec<(Mark, String)> {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((Mark::Same, a[i].clone()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push((Mark::Removed, a[i].clone()));
            i += 1;
        } else {
            out.push((Mark::Added, b[j].clone()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| (Mark::Removed, l.clone())));
    out.extend(b[j..].iter().map(|l| (Mark::Added, l.clone())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ccline-cc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SAMPLE: &str = r#"{
  "model": "opus",
  "statusLine": {
    "type": "command",
    "command": "bash ~/old.sh",
    "padding": 0,
    "futureKey": true
  },
  "env": {"A": "1"}
}
"#;

    #[test]
    fn edits_keep_order_and_unknown_keys() {
        let dir = temp("edit");
        let path = dir.join("settings.json");
        std::fs::write(&path, SAMPLE).unwrap();
        let base = Settings::load(&path).unwrap();
        assert_eq!(base.status_line().unwrap().command, "bash ~/old.sh");

        let mut s = base.clone();
        s.set_status_line(Some(&StatusLine {
            command: "'/stable home/agyline'".into(),
            padding: Some(0),
            enabled: Some(true),
            stack_with_default: Some(false),
        }));
        let text = s.render();
        let keys: Vec<usize> = ["\"model\"", "\"statusLine\"", "\"env\""]
            .iter()
            .map(|k| text.find(k).unwrap())
            .collect();
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "{text}");
        assert!(text.contains("\"futureKey\": true"));
        assert_eq!(s.changed_keys(&base), vec!["statusLine"]);

        let diff = s.diff(&base);
        assert!(diff.contains(&(Mark::Removed, "  \"command\": \"bash ~/old.sh\",".into())));
        assert!(text.contains("\"enabled\": true"));
        assert!(!text.contains("refreshInterval"));

        let backup = s.save(&base).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), SAMPLE);
        let again = Settings::load(&path).unwrap();
        assert_eq!(again.status_line().unwrap().enabled, Some(true));

        // Saving over a file that changed since is refused.
        assert!(matches!(s.save(&base), Err(SaveError::Changed)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn creates_a_missing_file_and_writes_through_links() {
        let dir = temp("link");
        let path = dir.join("settings.json");
        let base = Settings::load(&path).unwrap();
        assert_eq!(base.status_line(), None);
        let mut s = base.clone();
        s.set_status_line(Some(&StatusLine {
            command: "agyline".into(),
            ..StatusLine::default()
        }));
        assert_eq!(s.save(&base).unwrap(), None);
        assert!(
            s.render()
                .starts_with("{\n  \"statusLine\": {\n    \"type\": \"command\"")
        );

        #[cfg(unix)]
        {
            let real = dir.join("dotfiles.json");
            std::fs::rename(&path, &real).unwrap();
            std::os::unix::fs::symlink(&real, &path).unwrap();
            let base = Settings::load(&path).unwrap();
            let mut s = base.clone();
            s.set_status_line(Some(&StatusLine {
                command: "agyline --config".into(),
                ..StatusLine::default()
            }));
            s.save(&base).unwrap();
            assert!(
                std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert!(
                std::fs::read_to_string(&real)
                    .unwrap()
                    .contains("agyline --config")
            );
        }
        assert!(Settings::load(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn spots_agyline_commands() {
        assert!(is_ccline("'/home/tool/agyline'"));
        assert!(!is_ccline("/x/target/debug/ccometixline"));
        assert!(!is_ccline("bash ~/statusline.sh"));
    }
}
