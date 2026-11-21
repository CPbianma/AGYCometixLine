//! Render a dedicated quota line as one block per host-supplied ID prefix.

use super::{
    Ctx, Line, LineConfig, LineSpec, Report, Side, Span, Style, Theme, Token, lay_reading,
};
use crate::layout;
use crate::metrics::{self, Options};
use crate::render::ansi::sanitize;
use crate::render::width::{display_width, truncate};
use std::collections::BTreeMap;

pub(super) fn render(
    line: &LineConfig,
    context: &Ctx,
    theme: &Theme,
    width: Option<usize>,
) -> Option<(Vec<Line>, Report)> {
    let [entry] = line.left.as_slice() else {
        return None;
    };
    let config = entry.config();
    let options = config.options.as_ref()?;
    if config.metric != "quota"
        || !line.right.is_empty()
        || options.contains_key("bucket")
        || options.get("group_by").and_then(toml::Value::as_str) != Some("prefix")
    {
        return None;
    }
    let metric = metrics::find("quota")?;
    let mut families = BTreeMap::<String, Vec<(String, String)>>::new();
    for raw_id in context.input.quota.as_ref()?.keys() {
        let safe_id = sanitize(raw_id);
        let (family, label) = safe_id
            .split_once('-')
            .filter(|(family, label)| !family.is_empty() && !label.is_empty())
            .unwrap_or(("", &safe_id));
        families
            .entry(family.into())
            .or_default()
            .push((label.into(), raw_id.clone()));
    }
    let max_label_width = families
        .keys()
        .map(|family| display_width(family))
        .max()
        .unwrap_or(0)
        .min(width.map_or(24, |columns| (columns / 4).min(24)));
    let mut group_theme = theme.clone();
    group_theme.separator = Some("  ".into());
    let mut output = Vec::new();
    let mut report = Report::default();
    for (family, mut entries) in families {
        // Simple IDs before compound IDs, then lexical order, never inferred reset windows.
        entries.sort_by(|(left, _), (right, _)| {
            left.matches('-')
                .count()
                .cmp(&right.matches('-').count())
                .then(left.cmp(right))
        });
        let mut spec = LineSpec::default();
        for (label, raw_id) in entries {
            let mut lookup = Options::new();
            lookup.insert("bucket".into(), toml::Value::String(raw_id));
            if let Some(reading) = metrics::read(metric, context, &lookup) {
                let reading = reading
                    .set("name", &label)
                    .set("short", truncate(&label, 16))
                    .set("tiny", truncate(&label, 10));
                if let Some(item) = lay_reading(&config, theme, &metric.meta, &reading) {
                    spec.left.push(item);
                }
            }
        }
        let indent = if family.is_empty() || max_label_width == 0 {
            0
        } else {
            max_label_width + 2
        };
        let label = truncate(&family, max_label_width);
        let available = width.map(|columns| columns.saturating_sub(indent));
        report.natural = report
            .natural
            .max(indent + layout::natural(&spec, &group_theme));
        for (row_index, row) in layout::rows(&spec, available.unwrap_or(usize::MAX), &group_theme)
            .into_iter()
            .enumerate()
        {
            let plan = layout::plan(&spec.row(&row), available, &group_theme);
            report.steps.extend(
                plan.steps
                    .into_iter()
                    .map(|step| layout::Step { index: 0, ..step }),
            );
            if plan.line.is_empty() {
                continue;
            }
            // Starting the next family is intentional, not a width warning in the editor.
            if row_index > 0 {
                report.breaks.push((Side::Left, 0));
            }
            let mut rendered = Vec::new();
            if indent > 0 {
                rendered.push(Span::styled(
                    &label,
                    Style {
                        fg: theme.palette.color(Token::Quota),
                        ..Style::default()
                    },
                ));
                rendered.push(Span::raw(" ".repeat(indent - display_width(&label))));
            }
            rendered.extend(plan.line);
            output.push(rendered);
        }
    }
    if output.is_empty() {
        report.empty.push((Side::Left, 0));
    }
    Some((output, report))
}
