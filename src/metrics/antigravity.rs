//! Metrics backed only by Antigravity's native status-line contract.

use super::{Ctx, Metric, MetricMeta, OptKind, OptSpec, Options, Reading, fmt};
use crate::input::QuotaBucket;
use crate::render::ansi::sanitize;
use crate::render::width::{display_width, truncate};
use crate::style::{Category, GlyphTier, IconKey, Severity};
use crate::widgets::WidgetKind;

fn metadata(
    id: &'static str,
    name: &'static str,
    summary: &'static str,
    category: Category,
    icon: IconKey,
) -> MetricMeta {
    MetricMeta {
        id,
        name,
        summary,
        category,
        icon,
        widget: WidgetKind::Text,
        templates: ["{value}", "{value}", "{value}"],
        fields: &["value"],
        levels: &[],
        min_cc: None,
        needs_mod: false,
        options: &[],
        stateful: false,
        priority: 3,
    }
}

pub(super) fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            meta: MetricMeta {
                widget: WidgetKind::Bar,
                templates: [
                    "{name} {remaining}[ · reset {reset}]",
                    "{short} {remaining}",
                    "{tiny} {remaining}",
                ],
                fields: &[
                    "name",
                    "short",
                    "tiny",
                    "remaining",
                    "pct",
                    "reset",
                    "amount",
                    "compact_remaining",
                    "alert_reset",
                ],
                options: &[
                    OptSpec {
                        key: "bucket",
                        kind: OptKind::Text,
                        default: "all",
                        summary: "Exact quota bucket ID; unset expands all native buckets",
                    },
                    OptSpec {
                        key: "group_by",
                        kind: OptKind::Choice(&["none", "prefix"]),
                        default: "none",
                        summary: "Group by ID prefix on a dedicated left-side quota line",
                    },
                ],
                priority: 5,
                ..metadata(
                    "quota",
                    "Quota remaining",
                    "Native quota buckets, remaining capacity and reset time",
                    Category::Quota,
                    IconKey::Quota,
                )
            },
            compute: quota,
        },
        Metric {
            meta: metadata(
                "plan",
                "Subscription",
                "Native subscription tier (not inferred from quota)",
                Category::Identity,
                IconKey::Agent,
            ),
            compute: plan,
        },
        Metric {
            meta: metadata(
                "state",
                "Agent state",
                "Native agent state and pending approval",
                Category::Session,
                IconKey::Tools,
            ),
            compute: agent_state,
        },
        Metric {
            meta: metadata(
                "tasks",
                "Background tasks",
                "Number of running background tasks",
                Category::Session,
                IconKey::Tools,
            ),
            compute: tasks,
        },
        Metric {
            meta: metadata(
                "mode",
                "Execution mode",
                "Native execution mode when reported",
                Category::Identity,
                IconKey::OutputStyle,
            ),
            compute: mode,
        },
    ]
}

fn quota(context: &Ctx, options: &Options) -> Option<Reading> {
    let buckets = context.input.quota.as_ref()?;
    let (name, bucket) = match options.get("bucket").and_then(toml::Value::as_str) {
        Some(name) => (name, buckets.get(name)?),
        None => buckets
            .iter()
            .next()
            .map(|(name, bucket)| (name.as_str(), bucket))?,
    };
    let name = sanitize(name);
    let mut reading = Reading::default()
        .set("name", &name)
        .set("short", shorten(&name, 16, context.glyphs))
        .set("tiny", shorten(&name, 10, context.glyphs));
    if bucket.disabled == Some(true) {
        return Some(
            reading
                .set("remaining", "disabled")
                .set("compact_remaining", "disabled")
                .severity(Severity::Normal),
        );
    }
    if let Some(fraction) = bucket
        .remaining_fraction
        .filter(|fraction| fraction.is_finite() && (0.0..=1.0).contains(fraction))
    {
        let percentage = fmt::percent(fraction);
        reading = reading
            .set("remaining", format!("{percentage}% left"))
            .set("compact_remaining", format!("{percentage}%"))
            .set("pct", percentage)
            .ratio(fraction)
            .severity(if fraction <= 0.05 {
                Severity::Crit
            } else if fraction <= 0.2 {
                Severity::Warn
            } else {
                Severity::Normal
            });
    } else if let Some(amount) = bucket
        .remaining_amount
        .filter(|amount| amount.is_finite() && *amount >= 0.0)
    {
        let amount_text = if amount.fract() == 0.0 {
            format!("{amount:.0}")
        } else {
            format!("{amount:.2}")
        };
        reading = reading
            .set("amount", &amount_text)
            .set("remaining", format!("{amount_text} left"))
            .set("compact_remaining", format!("{amount_text} left"))
            .severity(if amount == 0.0 {
                Severity::Crit
            } else {
                Severity::Normal
            });
    } else {
        reading = reading
            .set("remaining", "unknown")
            .set("compact_remaining", "unknown")
            .severity(Severity::Normal);
    }
    if let Some(seconds) = reset_seconds(bucket, context.now) {
        let reset = if seconds <= 0 {
            "awaiting refresh".to_string()
        } else {
            fmt::countdown(seconds)
        };
        if matches!(reading.severity, Some(Severity::Warn | Severity::Crit)) {
            let marker = if context.glyphs == GlyphTier::Ascii {
                "reset"
            } else {
                "↻"
            };
            reading = reading.set("alert_reset", format!("{marker} {reset}"));
        }
        reading = reading.set("reset", reset);
    }
    Some(reading)
}

fn reset_seconds(bucket: &QuotaBucket, now: i64) -> Option<i64> {
    bucket
        .reset_time
        .as_deref()
        .and_then(|timestamp| chrono::DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.timestamp().saturating_sub(now))
        .or(bucket.reset_in_seconds)
}

fn shorten(name: &str, columns: usize, glyphs: GlyphTier) -> String {
    if display_width(name) <= columns {
        return name.to_string();
    }
    if glyphs == GlyphTier::Ascii {
        let prefix = truncate(name, columns.saturating_sub(2));
        format!("{}...", prefix.trim_end_matches('…'))
    } else {
        truncate(name, columns)
    }
}

fn plan(context: &Ctx, _: &Options) -> Option<Reading> {
    Some(Reading::default().set("value", sanitize(context.input.plan_tier.as_deref()?)))
}

fn agent_state(context: &Ctx, _: &Options) -> Option<Reading> {
    let state = if context.input.tool_confirmation_pending == Some(true) {
        "approval needed"
    } else {
        context.input.agent_state.as_deref()?
    };
    Some(Reading::default().set("value", sanitize(state)))
}

fn tasks(context: &Ctx, _: &Options) -> Option<Reading> {
    let count = context.input.task_count.filter(|count| *count > 0)?;
    Some(Reading::default().set("value", format!("{count} tasks")))
}

fn mode(context: &Ctx, _: &Options) -> Option<Reading> {
    Some(Reading::default().set("value", sanitize(context.input.cycle_mode.as_deref()?)))
}
