//! The terminal and Antigravity CLI environment the status line runs in.

use crate::input::Input;
use crate::style::ColorDepth;

/// Terminal width fallback from `COLUMNS`.
pub fn columns() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|c| *c > 0)
}

/// Color depth from the environment: `NO_COLOR` turns color off,
/// `COLORTERM=truecolor|24bit` allows RGB, a `*256color` TERM allows 256.
pub fn color_depth() -> ColorDepth {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return ColorDepth::None;
    }
    let colorterm = var("COLORTERM").to_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return ColorDepth::TrueColor;
    }
    if var("TERM").contains("256color") {
        return ColorDepth::Ansi256;
    }
    ColorDepth::Ansi16
}

/// AGY user settings. The type name is retained for internal compatibility.
/// Project Claude settings and Claude environment variables are never read.
#[derive(Clone, Debug, Default)]
pub struct ClaudeSettings(Vec<serde_json::Value>);

impl ClaudeSettings {
    pub fn load(_project_dir: Option<&str>) -> Self {
        Self::load_path(&crate::paths::agy_dir().join("settings.json"))
    }

    pub fn load_path(path: &std::path::Path) -> Self {
        ClaudeSettings(
            std::fs::read_to_string(path)
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .filter(serde_json::Value::is_object)
                .into_iter()
                .collect(),
        )
    }

    pub fn for_input(_input: &Input) -> Self {
        ClaudeSettings::load(None)
    }

    /// The first file that sets the value wins.
    fn find<T>(&self, get: impl Fn(&serde_json::Value) -> Option<T>) -> Option<T> {
        self.0.iter().find_map(get)
    }

    /// Vertical padding in AGY, never a deduction from horizontal width.
    pub fn statusline_vertical_padding(&self) -> usize {
        self.find(|j| j.get("statusLine")?.get("padding")?.as_u64())
            .unwrap_or(0) as usize
    }

    /// Internal horizontal-padding compatibility accessor. AGY's padding
    /// is vertical, so it must not enter the inherited width calculation.
    pub fn statusline_padding(&self) -> usize {
        0
    }

    /// AGY exposes no known auto-compaction threshold in its host settings.
    pub fn auto_compact(&self) -> AutoCompact {
        AutoCompact::default()
    }
}

/// Optional compaction metadata for the render pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AutoCompact {
    pub enabled: bool,
    /// The known compaction window, if the host ever reports one.
    pub window: Option<u64>,
    /// A host-reported compaction threshold, when known.
    pub pct: Option<f64>,
}
