//! Compatibility API for the upstream configurator. Network update checks
//! are disabled until this fork has its own publication endpoint.

use super::{dir, read_json};
use crate::config::UpdateConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The running agyline build.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

pub const RELEASES: &str = "https://github.com/Haleclipse/CCometixLine/releases";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateState {
    pub checked_at: i64,
    /// Newest version seen published, kept across failed checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// Whether the last check got an answer.
    #[serde(default)]
    pub ok: bool,
}

fn paths() -> (PathBuf, PathBuf) {
    (dir().join("update.json"), dir().join("update.lock"))
}

/// `(major, minor, patch, is_release)`; a pre-release sorts before its
/// release, and build metadata is ignored.
fn semver(v: &str) -> Option<(u64, u64, u64, bool)> {
    let v = v.trim().trim_start_matches('v').split('+').next()?;
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (
        parts.next()??,
        parts.next()??,
        parts.next()??,
        pre.is_none(),
    );
    parts.next().is_none().then_some(version)
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((semver(latest), semver(current)), (Some(l), Some(c)) if l > c)
}

/// No update source is configured for this fork.
pub fn available(_config: &UpdateConfig, _now: i64) -> Option<String> {
    // This fork has no publication endpoint. Never offer an upstream ccline
    // binary as an agyline update, or launch background network requests.
    None
}

/// The last check's result, without starting one.
pub fn last() -> UpdateState {
    read_json(&paths().0).unwrap_or_default()
}

/// Reserved for a future, separately configured agyline update endpoint.
pub fn fetch() -> Option<String> {
    None
}

/// No network request or state mutation takes place.
pub fn check(_now: i64) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("1.1.3", "1.1.2"));
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(is_newer("2.0.0", "2.0.0-dev.0"));
        assert!(!is_newer("1.1.2", "2.0.0-dev.0"));
        assert!(!is_newer("2.0.0-rc.1", "2.0.0"));
        assert!(!is_newer("2.0.0+build.5", "2.0.0"));
        assert!(!is_newer("1.1", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }
}
