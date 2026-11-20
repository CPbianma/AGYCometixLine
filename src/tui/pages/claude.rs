//! Antigravity integration. The filename and PageId retain upstream names.

use super::{Cx, Env, Outcome, Page};
use crate::cc_settings::{Settings, StatusLine};
use crate::integration::Installation;
use crate::tui::dialogs::{SettingsDiff, shown_path};
use crate::tui::doc::Doc;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

const ROW_COUNT: usize = 6;

#[derive(Default)]
pub struct ClaudePage {
    selected: usize,
    installation: Option<Installation>,
    base: Option<Result<Settings, String>>,
    draft: Option<Settings>,
}

impl ClaudePage {
    fn sync(&mut self, env: &Env, doc: &Doc) {
        self.installation = Some(env.installation(doc));
        let fresh =
            matches!(&self.base, Some(Ok(base)) if base.path == env.settings && !base.stale());
        if !fresh {
            self.base = Some(Settings::load(&env.settings));
            // A changed file invalidates a reviewed draft rather than silently
            // rebasing a host edit onto another writer's changes.
            self.draft = None;
        }
    }

    fn base(&self) -> Option<&Settings> {
        self.base.as_ref()?.as_ref().ok()
    }

    fn current(&self) -> Option<&Settings> {
        self.draft.as_ref().or_else(|| self.base())
    }

    fn diff(&self) -> Option<SettingsDiff> {
        let (base, draft) = (self.base()?, self.draft.as_ref()?);
        draft.differs(base).then(|| {
            SettingsDiff::install(
                self.installation.clone().expect("synced integration"),
                base.clone(),
                draft.clone(),
            )
        })
    }

    fn review(&self) -> Outcome {
        match self.diff() {
            Some(dialog) => Outcome::Open(Box::new(dialog)),
            None => Outcome::Toast("No changes to apply".into()),
        }
    }

    fn edit_line(&mut self, edit: impl FnOnce(&mut StatusLine)) -> Outcome {
        let Some(mut line) = self.current().and_then(Settings::status_line) else {
            return Outcome::Toast("Install/connect first".into());
        };
        let Some(installation) = &self.installation else {
            return Outcome::Ignored;
        };
        if installation.command().ok().as_ref() != Some(&line.command) {
            return Outcome::Toast("Install/connect before editing AGY options".into());
        }
        edit(&mut line);
        let mut draft = self.current().expect("settings loaded").clone();
        draft.set_status_line(Some(&line));
        self.draft = Some(draft);
        Outcome::Consumed
    }

    fn row(&self, index: usize, label: &str, value: String, focused: bool) -> Line<'static> {
        let selected = focused && index == self.selected;
        Line::from(vec![
            Span::styled(if selected { ui::SELECTED } else { "  " }, ui::accent()),
            Span::styled(
                format!("{label:<20}"),
                if selected {
                    ui::accent()
                } else {
                    Style::default()
                },
            ),
            Span::raw(value),
        ])
    }
}

impl Page for ClaudePage {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &Cx) {
        self.sync(context.env, context.doc);
        let [settings_area, details_area] =
            Layout::vertical([Constraint::Length(11), Constraint::Fill(1)])
                .spacing(1)
                .areas(area);
        let line = self
            .current()
            .and_then(Settings::status_line)
            .unwrap_or_default();
        let on_off = |value: Option<bool>, default: bool| {
            if value.unwrap_or(default) {
                "on"
            } else {
                "off"
            }
        };
        let mut rows = vec![
            self.row(
                0,
                "install / connect",
                "review stable executable + host settings".into(),
                context.focused,
            ),
            self.row(
                1,
                "vertical padding",
                line.padding.unwrap_or(0).to_string(),
                context.focused,
            ),
            self.row(
                2,
                "enabled",
                on_off(line.enabled, true).into(),
                context.focused,
            ),
            self.row(
                3,
                "stack_with_default",
                on_off(line.stack_with_default, false).into(),
                context.focused,
            ),
            self.row(
                4,
                "apply draft",
                if self.diff().is_some() {
                    "review and apply changes"
                } else {
                    "no changes"
                }
                .into(),
                context.focused,
            ),
            self.row(
                5,
                "uninstall / restore",
                "restore only the original statusLine".into(),
                context.focused,
            ),
            Line::raw(""),
        ];
        if let Some(Err(error)) = &self.base {
            rows.push(Line::styled(
                format!("Cannot edit settings: {error}"),
                Style::default().fg(ui::BAD),
            ));
        } else {
            rows.push(Line::styled(
                format!("Command: {}", line.command),
                ui::muted(),
            ));
        }
        frame.render_widget(
            Paragraph::new(rows).block(ui::panel("Antigravity statusLine")),
            settings_area,
        );

        let installation = context.env.installation(context.doc);
        let status = installation
            .status()
            .map(|status| status.to_string())
            .unwrap_or_else(|error| error);
        let mut details = vec![
            Line::styled(
                format!(
                    "AGY CLI version: {}",
                    context
                        .env
                        .agy_version()
                        .unwrap_or_else(|| "not seen yet".into())
                ),
                ui::strong(),
            ),
            Line::raw(format!("Settings: {}", shown_path(&installation.settings))),
            Line::raw(format!(
                "Stable executable: {}",
                shown_path(&installation.executable_path())
            )),
            Line::raw(format!(
                "Recovery record: {}",
                shown_path(&installation.record_path())
            )),
            Line::raw(""),
        ];
        details.extend(
            status
                .lines()
                .filter(|line| line.starts_with("agyline:") || line.starts_with("Installed at:"))
                .map(|line| Line::raw(line.to_string())),
        );
        details.extend([
            Line::raw(""),
            Line::styled(
                "Padding is vertical; it does not reduce available columns.",
                ui::muted(),
            ),
            Line::styled(
                "Unrelated settings and unknown statusLine keys are preserved.",
                ui::muted(),
            ),
            Line::styled(
                "No companion, subagent host settings or upstream update service.",
                ui::muted(),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(details)
                .wrap(Wrap { trim: false })
                .block(ui::panel("Installation & recovery")),
            details_area,
        );
    }

    fn handle(&mut self, key: KeyEvent, doc: &mut Doc, env: &Env) -> Outcome {
        self.sync(env, doc);
        match key.code {
            KeyCode::Up if self.selected == 0 => return Outcome::Ignored,
            KeyCode::Up => {
                self.selected -= 1;
                return Outcome::Consumed;
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(ROW_COUNT - 1);
                return Outcome::Consumed;
            }
            KeyCode::Char('a') => return self.review(),
            _ => {}
        }
        match (self.selected, key.code) {
            (0, KeyCode::Enter) => {
                let installation = env.installation(doc);
                match installation.preview_install() {
                    Ok((base, draft)) => {
                        Outcome::Open(Box::new(SettingsDiff::install(installation, base, draft)))
                    }
                    Err(error) => Outcome::Toast(error),
                }
            }
            (1, KeyCode::Left | KeyCode::Right) => self.edit_line(|line| {
                let padding = line.padding.unwrap_or(0);
                line.padding = Some(if key.code == KeyCode::Right {
                    padding.saturating_add(1).min(10)
                } else {
                    padding.saturating_sub(1)
                });
            }),
            (2, KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) => {
                self.edit_line(|line| line.enabled = Some(!line.enabled.unwrap_or(true)))
            }
            (3, KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) => self
                .edit_line(|line| {
                    line.stack_with_default = Some(!line.stack_with_default.unwrap_or(false))
                }),
            (4, KeyCode::Enter) => self.review(),
            (5, KeyCode::Enter) => match SettingsDiff::uninstall(env.installation(doc)) {
                Ok(dialog) => Outcome::Open(Box::new(dialog)),
                Err(error) => Outcome::Toast(error),
            },
            _ => Outcome::Ignored,
        }
    }

    fn keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("up/down", "move"),
            ("left/right", "change"),
            ("enter", "review"),
            ("a", "apply draft"),
        ]
    }

    fn preview(&self) -> bool {
        false
    }
    fn file(&self) -> &'static str {
        "settings.json"
    }
    fn unapplied(&self) -> Option<SettingsDiff> {
        self.diff()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::app::tests::{ctrl, env, key, screen};
    use crate::tui::doc::tests::temp_paths;
    use crate::tui::pages::PageId;
    use serde_json::{Value, json};

    #[test]
    fn connects_edits_and_restores_through_the_same_installation_record() {
        let (directory, paths) = temp_paths("agy-integration");
        let settings = directory.join("settings.json");
        let original = json!({"type":"command", "command":"old tool", "futureKey":true});
        std::fs::write(
            &settings,
            json!({"statusLine": original, "keep": 42}).to_string(),
        )
        .unwrap();
        let mut app = App::new(
            Doc::open(paths),
            Env {
                settings: settings.clone(),
                ..env()
            },
        );
        app.open_page(PageId::Claude);
        let text = screen(&mut app, 112, 33);
        assert!(
            text.contains("Antigravity statusLine") && text.contains("vertical padding"),
            "{text}"
        );
        assert!(!text.contains("refreshInterval") && !text.contains("Claude Code"));
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('y')));
        assert!(directory.join("installation.json").is_file());
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Right));
        app.handle_key(ctrl('s'));
        assert!(screen(&mut app, 112, 33).contains("Change "));
        app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(
            Settings::load(&settings)
                .unwrap()
                .status_line()
                .unwrap()
                .padding,
            Some(1)
        );
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Down));
        }
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Char('y')));
        let restored: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored["statusLine"], original);
        assert_eq!(restored["keep"], 42);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
