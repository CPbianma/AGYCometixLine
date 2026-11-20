//! F · Changing AGY `settings.json`: the change as a diff, then
//! a backup and the write.

use super::{Action, Dialog, Reply};
use crate::cc_settings::{Mark, Settings};
use crate::integration::Installation;
use crate::tui::doc::Doc;
use crate::tui::pages::Cx;
use crate::tui::ui;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const WIDTH: u16 = 72;
/// Most diff lines shown.
const MAX_LINES: usize = 16;

pub struct SettingsDiff {
    /// The file as read.
    pub base: Settings,
    /// The settings to write.
    pub draft: Settings,
    /// Quit once they're written: the last step of saving on the way out.
    pub then_quit: bool,
    installation: Installation,
    uninstall: bool,
}

impl SettingsDiff {
    pub fn install(installation: Installation, base: Settings, draft: Settings) -> Self {
        SettingsDiff {
            base,
            draft,
            then_quit: false,
            installation,
            uninstall: false,
        }
    }

    pub fn uninstall(installation: Installation) -> Result<Self, String> {
        let (base, draft) = installation.preview_uninstall()?;
        let mut dialog = Self::install(installation, base, draft);
        dialog.uninstall = true;
        Ok(dialog)
    }
}

/// `path` with the home directory as `~`.
pub fn shown_path(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|h| path.strip_prefix(h).ok().map(|p| p.to_path_buf())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

impl Dialog for SettingsDiff {
    fn render(&mut self, f: &mut Frame, area: Rect, _: &Cx) {
        let diff = self.draft.diff(&self.base);
        let shown = diff.len().min(MAX_LINES);
        let more = u16::from(diff.len() > shown);
        let rect = ui::popup(f, area, WIDTH, shown as u16 + more + 7);
        let room = WIDTH as usize - 2;
        let mut text = vec![
            Line::raw(if self.uninstall {
                " Restore only the recorded AGY statusLine:"
            } else {
                " Install stable agyline and apply this AGY statusLine:"
            }),
            Line::raw(""),
        ];
        for (mark, line) in diff.iter().take(shown) {
            let (sign, style) = match mark {
                Mark::Same => ("  ", Style::default()),
                Mark::Added => ("+ ", Style::default().fg(ui::OK)),
                Mark::Removed => ("- ", Style::default().fg(ui::BAD)),
            };
            text.push(Line::from(ui::clip(
                vec![Span::styled(format!("  {sign}{line}"), style)],
                room,
            )));
        }
        if diff.len() > shown {
            text.push(Line::styled(
                format!("    … {} more lines", diff.len() - shown),
                ui::muted(),
            ));
        }
        text.push(Line::raw(""));
        text.push(Line::from(vec![
            Span::styled(" Recovery  ", ui::muted()),
            Span::raw("installation.json + unique settings backup"),
        ]));
        let apply = if self.then_quit {
            "apply and quit"
        } else {
            "apply"
        };
        text.push(ui::hints(&[("Y", apply), ("N", "cancel")]));
        let title = format!("Change {}", shown_path(&self.base.path));
        f.render_widget(Paragraph::new(text).block(ui::panel(&title)), rect);
    }

    fn handle(&mut self, key: KeyEvent, _: &mut Doc) -> Reply {
        match key.code {
            KeyCode::Char('y' | 'Y') => match if self.uninstall {
                self.installation.apply_uninstall(&self.base)
            } else {
                self.installation.apply_install(&self.base, &self.draft)
            } {
                Ok(_) if self.then_quit => Reply::Then(Action::Quit),
                Ok(status) => Reply::Then(Action::Toast(format!("agyline: {}", status.state))),
                Err(e) => Reply::Then(Action::Error(format!("Couldn't write settings.json: {e}"))),
            },
            KeyCode::Char('n' | 'N') | KeyCode::Esc => Reply::Close,
            _ => Reply::Stay,
        }
    }
}
