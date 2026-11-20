use ccometixline::cli::{Cli, Command, PreviewArgs};
use ccometixline::config::{Config, DepthSetting, LoadNote};
use ccometixline::env::{AutoCompact, ClaudeSettings};
use ccometixline::input::{Input, MAX_INPUT_BYTES, SubagentInput};
use ccometixline::metrics::{Commands, GitSource};
use ccometixline::models::Registry;
use ccometixline::render::ansi::{self, EmitOptions};
use ccometixline::render::span::{Span, Style};
use ccometixline::scenarios;
use ccometixline::state::{self, StateView};
use ccometixline::statusline::{self, Frame};
use ccometixline::style::palette::Token;
use chrono::{Local, Utc};
use std::io::{self, IsTerminal, Read};

fn main() {
    let cli = Cli::parse_args();

    match cli.command {
        Some(Command::Preview(args)) => return preview(args),
        Some(Command::Install(args)) => return integration_result(args.installation().install()),
        Some(Command::Status(args)) => return integration_result(args.installation().status()),
        Some(Command::Uninstall(args)) => {
            return integration_result(args.installation().uninstall());
        }
        Some(Command::RefreshGit { dir }) => return state::git_cache::refresh(&dir),
        Some(Command::UpdateCheck) => {
            return;
        }
        Some(Command::RefreshCmd {
            cwd,
            session,
            timeout_ms,
            run,
        }) => {
            let timeout = std::time::Duration::from_millis(timeout_ms);
            return state::cmd_cache::refresh(&run, cwd.as_deref(), session.as_deref(), timeout);
        }
        None => {}
    }

    if cli.config || io::stdin().is_terminal() {
        if !io::stdout().is_terminal() {
            eprintln!("agyline: the configuration TUI needs a terminal.");
            std::process::exit(1);
        }
        if let Err(e) = ccometixline::tui::run() {
            eprintln!("agyline: {e}");
            std::process::exit(1);
        }
        return;
    }

    run(cli.subagent, cli.capture.as_deref());
}

fn run(subagent: bool, capture: Option<&std::path::Path>) {
    let mut text = String::new();
    if io::stdin()
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .is_err()
        || text.len() > MAX_INPUT_BYTES
    {
        println!("\x1b[0magyline: could not read status line input\x1b[0m");
        return;
    }
    let input = match Input::parse(&text) {
        Ok(input) => input,
        Err(_) => {
            println!("\x1b[0magyline: invalid status line input\x1b[0m");
            return;
        }
    };
    if let Some(path) = capture
        && let Some(snapshot) = input.replay_value()
        && state::write_json(path, &snapshot).is_err()
    {
        eprintln!("agyline: diagnostic snapshot could not be written");
    }
    if !input.is_antigravity() {
        let tasks = SubagentInput::detect(&text).or_else(|| subagent.then(SubagentInput::default));
        if let Some(payload) = tasks {
            return run_subagent(&payload, &text);
        }
    }
    let (config, note) = Config::load();
    let registry = Registry::load();
    let settings = ClaudeSettings::for_input(&input);
    let (now, tz) = (Utc::now().timestamp(), *Local::now().offset());
    let view = state::observe(&input, &text, &registry, now, tz);
    let frame = Frame {
        now,
        tz,
        width: statusline::budget_for_input(&config, &settings, &input),
        git: GitSource::Cached(config.git.cache_ms),
        state: view,
        compact: settings.auto_compact(),
        commands: Commands::Run {
            stdin: text.clone(),
        },
    };
    let mut lines = statusline::lines(&input, &config, &registry, frame);
    if let LoadNote::Invalid(_) = note {
        let warn = statusline::theme(&config).palette.color(Token::Warn);
        let mark = Span::styled(
            " ! agyline config",
            Style {
                fg: warn,
                ..Style::default()
            },
        );
        match lines.first_mut() {
            Some(first) => first.push(mark),
            None => lines.push(vec![mark]),
        }
    }
    let opts = EmitOptions {
        depth: config.color_depth.resolve(),
        guards: true,
    };
    println!("{}", ansi::emit(&lines, opts));
}

/// `subagentStatusLine`: one JSON object per task, `{"id", "content"}`.
/// Tasks whose line comes out empty are left out, so Claude Code keeps its
/// own text for them (an empty `content` would hide the task).
fn run_subagent(payload: &SubagentInput, raw: &str) {
    let (config, _) = Config::load();
    let registry = Registry::load();
    let frame = Frame {
        now: Utc::now().timestamp(),
        tz: *Local::now().offset(),
        width: None,
        git: GitSource::Cached(config.git.cache_ms),
        state: StateView::default(),
        compact: AutoCompact::default(),
        commands: Commands::Run {
            stdin: raw.to_string(),
        },
    };
    let opts = EmitOptions {
        depth: config.color_depth.resolve(),
        guards: false,
    };
    for (id, line) in statusline::subagent_lines(payload, &config, &registry, frame) {
        let content = ansi::emit(&[line], opts);
        if !content.is_empty() {
            println!("{}", serde_json::json!({ "id": id, "content": content }));
        }
    }
}

fn integration_result(result: Result<ccometixline::integration::Status, String>) {
    match result {
        Ok(status) => println!("{status}"),
        Err(error) => {
            eprintln!("agyline: {error}");
            std::process::exit(1);
        }
    }
}

fn preview(args: PreviewArgs) {
    if args.list {
        println!("Scenarios:");
        for s in scenarios::SCENARIOS {
            println!("  {:<12} {}", s.name, s.summary);
        }
        return;
    }
    let (config, _) = Config::load();
    let depth = match args.color.as_str() {
        "truecolor" => DepthSetting::TrueColor,
        "256" => DepthSetting::Ansi256,
        "16" => DepthSetting::Ansi16,
        "none" => DepthSetting::None,
        _ => config.color_depth,
    }
    .resolve();
    let width = args
        .width
        .or_else(ccometixline::env::columns)
        .unwrap_or(100);
    let registry = Registry::load();
    let now = Utc::now().timestamp();
    let tz = *Local::now().offset();
    let opts = EmitOptions {
        depth,
        guards: true,
    };

    let render = |input: &Input, git: GitSource, state: StateView, compact: AutoCompact| {
        let frame = Frame {
            now,
            tz,
            width: Some(width),
            git,
            state,
            compact,
            commands: Commands::Show,
        };
        ansi::emit(&statusline::lines(input, &config, &registry, frame), opts)
    };
    let scenario = |s: &scenarios::Scenario| {
        render(
            &s.input(now),
            GitSource::Fixed(s.git()),
            s.state(now),
            AutoCompact::default(),
        )
    };

    if args.subagent {
        let frame = Frame {
            now,
            tz,
            width: Some(width),
            git: GitSource::Fixed(None),
            state: StateView::default(),
            compact: AutoCompact::default(),
            commands: Commands::Show,
        };
        let mut payload = scenarios::subagent(now);
        payload.columns = Some(width);
        for (id, line) in statusline::subagent_lines(&payload, &config, &registry, frame) {
            println!("\x1b[2m{id:>3} │\x1b[0m {}", ansi::emit(&[line], opts));
        }
        return;
    }
    if args.all {
        for s in scenarios::SCENARIOS {
            println!("\x1b[2m── {} · {}\x1b[0m", s.name, s.summary);
            println!("{}\n", scenario(s));
        }
        return;
    }
    if let Some(s) = scenarios::find(&args.scenario) {
        println!("{}", scenario(s));
        return;
    }
    match std::fs::read_to_string(&args.scenario).map(|t| Input::parse(&t)) {
        Ok(Ok(input)) => {
            // A real payload renders against the live environment, but
            // previewing must not record anything.
            let compact = ClaudeSettings::for_input(&input).auto_compact();
            println!(
                "{}",
                render(&input, GitSource::Live, StateView::default(), compact)
            )
        }
        Ok(Err(_)) => eprintln!("agyline: {} is not status line JSON", args.scenario),
        Err(_) => eprintln!(
            "agyline: no scenario or file named {:?}; see `agyline preview --list`",
            args.scenario
        ),
    }
}
