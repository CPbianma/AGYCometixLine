use ccometixline::config::{Config, ItemSpec, LineConfig};
use ccometixline::env::AutoCompact;
use ccometixline::input::Input;
use ccometixline::metrics::{self, Commands, Ctx, GitSource, Options, Reading};
use ccometixline::models::Registry;
use ccometixline::render::{ansi, width};
use ccometixline::state::StateView;
use ccometixline::statusline::{self, Frame};
use ccometixline::style::{GlyphTier, Severity, SkinKind};
use chrono::FixedOffset;

const NOW: i64 = 1_795_132_800;

fn payload(extra: serde_json::Value) -> Input {
    let mut value = serde_json::json!({
        "product": "antigravity", "version": "1.5.13",
        "conversation_id": "test-session", "conversation_title": "Test session",
        "model": {"id": "gemini-test", "display_name": "Gemini Test", "effort": "xhigh"},
        "terminal_width": 100, "cwd": "/workspace/project"
    });
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    Input::parse(&value.to_string()).unwrap()
}

fn reading(input: &Input, metric: &str, options: Options) -> Option<Reading> {
    let registry = Registry::default();
    let context = Ctx::new(
        input,
        &registry,
        NOW,
        FixedOffset::east_opt(0).unwrap(),
        GlyphTier::Ascii,
    )
    .with_git(GitSource::Fixed(None));
    let metric = metrics::find(metric).expect("native metric must be registered");
    metrics::read(metric, &context, &options)
}

fn bucket(name: &str) -> Options {
    let mut options = Options::new();
    options.insert("bucket".into(), toml::Value::String(name.into()));
    options
}

fn render(input: &Input, skin: SkinKind, columns: usize) -> String {
    let config = Config {
        skin,
        glyphs: GlyphTier::Unicode,
        lines: vec![
            LineConfig {
                left: vec![ItemSpec::Id("model".into()), ItemSpec::Id("context".into())],
                right: vec![],
            },
            LineConfig {
                left: vec![ItemSpec::Id("plan".into()), ItemSpec::Id("quota".into())],
                right: vec![],
            },
        ],
        ..Config::default()
    };
    let frame = Frame {
        now: NOW,
        tz: FixedOffset::east_opt(0).unwrap(),
        width: Some(columns),
        git: GitSource::Fixed(None),
        state: StateView::default(),
        compact: AutoCompact::default(),
        commands: Commands::Show,
    };
    ansi::emit(
        &statusline::lines(input, &config, &Registry::default(), frame),
        ansi::EmitOptions::plain(),
    )
}

#[test]
fn normalizes_native_conversation_and_nested_effort() {
    let input = payload(serde_json::json!({}));
    assert_eq!(input.session_id.as_deref(), Some("test-session"));
    assert_eq!(input.session_name.as_deref(), Some("Test session"));
    let model = reading(&input, "model", Options::new()).unwrap();
    assert_eq!(model.get("effort").unwrap().text, "xHigh");
}

#[test]
fn native_display_name_does_not_repeat_its_effort_suffix() {
    let input = payload(serde_json::json!({"model": {
        "id": "Gemini 4 Neon (XHigh)", "display_name": "Gemini 4 Neon (XHigh)", "effort": "xhigh"
    }}));
    let model = reading(&input, "model", Options::new()).unwrap();
    assert_eq!(model.get("name").unwrap().text, "Gemini 4 Neon");
    assert_eq!(model.get("effort").unwrap().text, "xHigh");
}

#[test]
fn narrow_quota_preserves_the_remaining_value_for_long_native_bucket_ids() {
    let input = payload(serde_json::json!({"quota": {
        "gemini-argon-wtu-weekly": {"remaining_fraction": 0.75}
    }}));
    for skin in [
        SkinKind::Plain,
        SkinKind::Bracket,
        SkinKind::Pill,
        SkinKind::Powerline,
    ] {
        let output = render(&input, skin, 32);
        assert!(output.contains("75% left"), "{skin:?}: {output}");
    }
}

#[test]
fn native_versions_are_not_compared_with_claude_versions() {
    let input = payload(serde_json::json!({}));
    let output = render(&input, SkinKind::Plain, 100);
    assert!(output.contains("Gemini Test"), "{output}");
    assert!(!output.contains("needs Claude"), "{output}");
}

#[test]
fn host_context_percentage_wins_over_registry_guesses() {
    let input = payload(serde_json::json!({
        "model": {"id": "kimi-k2", "display_name": "Kimi", "effort": "high"},
        "context_window": {"total_input_tokens": 300000, "context_window_size": 1000000, "used_percentage": 12.5}
    }));
    let context = reading(&input, "context", Options::new()).unwrap();
    assert_eq!(context.ratio, Some(0.125));
    assert_eq!(context.get("pct").unwrap().text, "13");
}

#[test]
fn context_can_show_percentage_without_inventing_token_counts_or_window() {
    let input = payload(
        serde_json::json!({"context_window": {"context_window_size": 0, "used_percentage": 25}}),
    );
    let context = reading(&input, "context", Options::new()).unwrap();
    assert_eq!(context.ratio, Some(0.25));
    assert!(context.get("tokens").is_none());
    assert!(context.get("window").is_none());
}

#[test]
fn native_context_never_uses_cumulative_input_as_current_occupancy() {
    let input = payload(
        serde_json::json!({"context_window": {"total_input_tokens": 9000000, "context_window_size": 1000000}}),
    );
    assert!(reading(&input, "context", Options::new()).is_none());
}

#[test]
fn quota_is_remaining_with_low_remaining_warning_polarity() {
    for (fraction, severity) in [
        (0.82, Severity::Normal),
        (0.15, Severity::Warn),
        (0.0, Severity::Crit),
    ] {
        let input = payload(
            serde_json::json!({"quota": {"gemini-weekly": {"remaining_fraction": fraction, "reset_in_seconds": 7200}}}),
        );
        let quota = reading(&input, "quota", bucket("gemini-weekly")).unwrap();
        assert_eq!(quota.ratio, Some(fraction));
        assert_eq!(quota.severity, Some(severity));
        assert_eq!(quota.get("name").unwrap().text, "gemini-weekly");
        assert!(quota.get("remaining").unwrap().text.contains('%'));
        assert_eq!(quota.get("reset").unwrap().text, "2h00m");
    }
}

#[test]
fn quotas_keep_amounts_disabled_missing_and_bad_buckets_distinct() {
    let input = payload(serde_json::json!({"quota": {
        "amount": {"remaining_amount": 1250}, "disabled": {"remaining_fraction": 1, "disabled": true},
        "invalid": {"remaining_fraction": 2}, "negative": {"remaining_amount": -10},
        "unknown": {}, "broken": "unexpected", "healthy": {"remaining_fraction": 0.5}
    }}));
    let amount = reading(&input, "quota", bucket("amount")).unwrap();
    assert_eq!(amount.get("remaining").unwrap().text, "1250 left");
    assert_eq!(amount.ratio, None);
    for name in ["invalid", "negative", "unknown", "broken"] {
        let quota = reading(&input, "quota", bucket(name)).unwrap();
        assert_eq!(quota.get("remaining").unwrap().text, "unknown", "{name}");
        assert_eq!(quota.ratio, None);
    }
    let disabled = reading(&input, "quota", bucket("disabled")).unwrap();
    assert_eq!(disabled.get("remaining").unwrap().text, "disabled");
    assert_eq!(disabled.ratio, None);
    assert_eq!(
        reading(&input, "quota", bucket("healthy")).unwrap().ratio,
        Some(0.5)
    );
    assert!(reading(&input, "quota", bucket("absent")).is_none());
}

#[test]
fn expired_reset_does_not_fabricate_a_refill() {
    let input = payload(
        serde_json::json!({"quota": {"weekly": {"remaining_fraction": 0.0, "reset_time": "2020-01-01T00:00:00Z"}}}),
    );
    let quota = reading(&input, "quota", bucket("weekly")).unwrap();
    assert_eq!(quota.ratio, Some(0.0));
    assert_eq!(quota.get("reset").unwrap().text, "awaiting refresh");
}

#[test]
fn native_cost_is_clearly_an_estimate_not_a_subscription_bill() {
    let input = payload(serde_json::json!({"cost": {"total_usd": 0.42, "estimated": false}}));
    let cost = reading(&input, "cost", Options::new()).unwrap();
    assert_eq!(cost.get("usd").unwrap().text, "~$0.42");
}

#[test]
fn unsupported_claude_metrics_are_hidden_even_with_misleading_fields() {
    let input = payload(
        serde_json::json!({"rate_limits": {"five_hour": {"used_percentage": 30}}, "context_window": {"total_input_tokens": 10000, "context_window_size": 200000}, "cost": {"total_usd": 2}}),
    );
    for metric in [
        "quota.5h",
        "quota.7d",
        "quota.eta",
        "context.to_compact",
        "cache.cold_cost",
        "cost.rate",
        "cost.today",
    ] {
        assert!(
            reading(&input, metric, Options::new()).is_none(),
            "{metric}"
        );
    }
}

#[test]
fn all_quota_buckets_expand_independently_and_fit_all_skins() {
    let input = payload(serde_json::json!({"plan_tier": "Pro", "quota": {
        "gemini-weekly": {"remaining_fraction": 0.82, "reset_in_seconds": 7200},
        "claude-weekly": {"remaining_fraction": 0.15, "reset_in_seconds": 3600},
        "credits": {"remaining_amount": 1250}
    }}));
    for skin in [
        SkinKind::Plain,
        SkinKind::Bracket,
        SkinKind::Pill,
        SkinKind::Powerline,
    ] {
        for columns in [32, 60, 100, 160] {
            let output = render(&input, skin, columns);
            assert!(!output.is_empty());
            for line in output.lines() {
                assert!(width::display_width(line) <= columns, "{columns}: {line}");
            }
        }
        let output = render(&input, skin, 160);
        for name in ["gemini-weekly", "claude-weekly", "credits", "Pro"] {
            assert!(output.contains(name), "{name}: {output}");
        }
    }
}

#[test]
fn untrusted_bucket_names_cannot_inject_terminal_controls() {
    let input = payload(
        serde_json::json!({"quota": {"evil\u{1b}]0;title\u{7}\n": {"remaining_fraction": 0.5}}}),
    );
    let output = render(&input, SkinKind::Plain, 160);
    assert!(!output.contains('\u{1b}'));
    assert!(!output.contains('\u{7}'));
}

#[test]
fn replay_excludes_account_data_unknown_fields_and_nested_secrets() {
    let input = payload(serde_json::json!({
        "email": "private@example.invalid", "access_token": "must-not-persist",
        "messages": [{"text": "private prompt"}],
        "model": {"id": "test", "display_name": "Test", "access_token": "nested-secret"},
        "quota": {"weekly": {"remaining_fraction": 0.5, "api_key": "bucket-secret"}}
    }));
    let snapshot = input.replay_value().unwrap().to_string();
    for secret in [
        "private@example.invalid",
        "must-not-persist",
        "private prompt",
        "nested-secret",
        "bucket-secret",
    ] {
        assert!(!snapshot.contains(secret));
    }
    let replay = Input::parse(&snapshot).unwrap();
    assert!(replay.is_antigravity());
    assert_eq!(replay.session_id, input.session_id);
    assert_eq!(
        reading(&replay, "quota", bucket("weekly")).unwrap().ratio,
        Some(0.5)
    );
}

#[test]
fn native_width_uses_current_payload_not_vertical_host_padding() {
    let input = payload(serde_json::json!({"terminal_width": 88}));
    let mut configuration = Config::default();
    configuration.layout.reserve = 2;
    let settings = ccometixline::env::ClaudeSettings::default();
    assert_eq!(
        statusline::budget_for_input(&configuration, &settings, &input),
        Some(86)
    );
    configuration.layout.width.0 = Some(60);
    assert_eq!(
        statusline::budget_for_input(&configuration, &settings, &input),
        Some(58)
    );
}

#[test]
fn malformed_optional_quota_field_keeps_other_fields_and_buckets() {
    let input = payload(serde_json::json!({"quota": {
        "weekly": {"remaining_fraction": "bad", "remaining_amount": 5, "reset_in_seconds": 120},
        "healthy": {"remaining_fraction": 0.8}
    }}));
    let weekly = reading(&input, "quota", bucket("weekly")).unwrap();
    assert_eq!(weekly.get("remaining").unwrap().text, "5 left");
    assert_eq!(weekly.get("reset").unwrap().text, "2:00");
    assert_eq!(
        reading(&input, "quota", bucket("healthy")).unwrap().ratio,
        Some(0.8)
    );
}

#[test]
fn replay_round_trip_does_not_turn_legacy_null_fields_into_native_markers() {
    let input = Input::parse(r#"{"version":"2.1.305","model":{"id":"claude-test"}}"#).unwrap();
    assert!(!input.is_antigravity());
    let replay = Input::parse(&input.replay_value().unwrap().to_string()).unwrap();
    assert!(!replay.is_antigravity());
    assert_eq!(replay.cc_version(), input.cc_version());
}

#[test]
fn replay_redacts_email_like_dynamic_bucket_identifiers() {
    let input = payload(
        serde_json::json!({"quota": {"private@example.invalid": {"remaining_fraction": 0.25}}}),
    );
    let snapshot = input.replay_value().unwrap().to_string();
    assert!(!snapshot.contains("private@example.invalid"));
    assert_eq!(
        input.quota.as_ref().unwrap()["private@example.invalid"].remaining_fraction,
        Some(0.25)
    );
    let replay = Input::parse(&snapshot).unwrap();
    assert_eq!(replay.quota.as_ref().unwrap().len(), 1);
}

#[test]
fn native_history_never_records_claude_fixed_window_fields() {
    let input = payload(
        serde_json::json!({"rate_limits": {"five_hour": {"used_percentage": 30, "resets_at": 1795200000}, "seven_day": {"used_percentage": 50, "resets_at": 1795200000}}}),
    );
    let sample = ccometixline::state::Sample::from_input(&input, NOW);
    assert_eq!(
        (sample.h5, sample.h5_reset, sample.d7, sample.d7_reset),
        (None, None, None, None)
    );
}
