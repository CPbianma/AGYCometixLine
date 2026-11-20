//! The six pages. Each draws its body and handles the keys it understands;
//! anything it ignores falls through to the app's global keys.

mod claude;
mod home;
mod layout;
mod models;
mod presets;
mod style;

use super::dialogs::{Dialog, SettingsDiff};
use super::doc::Doc;
use super::preview::Target;
use crate::state::SessionState;
use crate::state::models_seen::Seen;
use crate::version::Version;
use chrono::FixedOffset;
use ratatui::Frame;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

pub use claude::ClaudePage;
pub use home::Home;
pub use layout::LayoutPage;
pub use models::Models;
pub use presets::Presets;
pub use style::StylePage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageId {
    Home,
    Presets,
    Layout,
    Style,
    Models,
    Claude,
}

impl PageId {
    pub const ALL: [PageId; 6] = [
        PageId::Home,
        PageId::Presets,
        PageId::Layout,
        PageId::Style,
        PageId::Models,
        PageId::Claude,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            PageId::Home => "Home",
            PageId::Presets => "Presets",
            PageId::Layout => "Layout",
            PageId::Style => "Style",
            PageId::Models => "Models",
            PageId::Claude => "Antigravity",
        }
    }
}

/// What the TUI knows about its surroundings.
pub struct Env {
    /// Unix epoch seconds at start.
    pub now: i64,
    pub tz: FixedOffset,
    /// The newest session ccline recorded, for replay and version checks.
    pub session: Option<SessionState>,
    /// Model ids seen in recent sessions.
    pub seen: BTreeMap<String, Seen>,
    /// Antigravity CLI user settings file.
    pub settings: std::path::PathBuf,
    /// The last update check.
    #[allow(dead_code)]
    // Compatibility with upstream TUI fixtures; never shown as agyline releases.
    pub update: crate::state::update::UpdateState,
}

impl Env {
    pub fn detect() -> Env {
        Env {
            now: chrono::Utc::now().timestamp(),
            tz: *chrono::Local::now().offset(),
            session: crate::state::session::latest(),
            seen: crate::state::models_seen::load(),
            settings: crate::cc_settings::Settings::user_path(),
            update: crate::state::update::last(),
        }
    }

    pub fn installation(&self, doc: &Doc) -> crate::integration::Installation {
        crate::integration::Installation::new(
            self.settings.clone(),
            doc.paths()
                .config
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(crate::paths::home),
        )
    }

    /// Internal legacy capability gate; AGY must never be compared with
    /// inherited Claude version thresholds.
    pub fn cc_version(&self) -> Option<Version> {
        let value = self.session.as_ref()?.last_input.as_ref()?;
        crate::input::Input::parse(&value.to_string())
            .ok()?
            .cc_version()
    }

    /// The actual AGY CLI version from the most recent native render.
    pub fn agy_version(&self) -> Option<String> {
        let value = self.session.as_ref()?.last_input.as_ref()?;
        let input = crate::input::Input::parse(&value.to_string()).ok()?;
        if !input.is_antigravity() {
            return None;
        }
        input.version
    }
}

/// Read-only context for drawing.
pub struct Cx<'a> {
    pub doc: &'a Doc,
    pub env: &'a Env,
    /// The preview bar's scenario and overrides, for pages that render.
    pub preview: &'a super::preview::Preview,
    /// Whether the page has the keys (else the tab row does), so it shows
    /// its selection.
    pub focused: bool,
}

pub enum Outcome {
    /// Not handled; the app's global keys get it.
    Ignored,
    Consumed,
    Goto(PageId),
    Open(Box<dyn Dialog>),
    Toast(String),
    /// Put this text on the clipboard.
    Copy(String),
    Quit,
}

pub trait Page {
    fn render(&mut self, f: &mut Frame, area: Rect, cx: &Cx);
    /// Handle a key. Return `Ignored` for Up at the top of the page so
    /// focus moves to the tab row.
    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome;
    /// Key hints for the footer.
    fn keys(&self) -> Vec<(&'static str, &'static str)>;
    /// Whether the preview bar shows under this page.
    fn preview(&self) -> bool {
        true
    }
    /// The file this page edits, for the title bar.
    fn file(&self) -> &'static str {
        "config.toml"
    }
    /// Pasted text; returns whether a field took it.
    fn paste(&mut self, _text: &str) -> bool {
        false
    }
    /// The preview scenario that shows this page's effect best.
    fn scenario(&self) -> Option<&'static str> {
        None
    }
    /// Which lines the preview bar shows.
    fn target(&self) -> Target {
        Target::Status
    }
    /// A config the preview bar shows instead of the edited one, and its
    /// name, while the page has the keys (a preset under the cursor).
    fn previewing(&self, _doc: &Doc) -> Option<(String, crate::config::Config)> {
        None
    }
    /// Whether letter keys reach the page while the tab row has the keys,
    /// for pages whose letters only open things.
    fn shortcuts(&self) -> bool {
        false
    }
    /// Changes to AGY settings drafted here and not written yet,
    /// for saving and quitting to bring up.
    fn unapplied(&self) -> Option<SettingsDiff> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn displays_only_the_native_agy_cli_version() {
        let mut environment = crate::tui::app::tests::env();
        environment.session = Some(SessionState {
            last_input: Some(json!({"product":"antigravity", "version":"0.4.7"})),
            ..SessionState::default()
        });
        assert_eq!(environment.agy_version().as_deref(), Some("0.4.7"));
        assert_eq!(environment.cc_version(), None);
        environment.session.as_mut().unwrap().last_input = Some(json!({"version":"2.1.300"}));
        assert_eq!(environment.agy_version(), None);
    }
}
