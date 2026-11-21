use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "agyline-smoke-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn render(&self, input: &str, arguments: &[&str]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_agyline"))
            .args(arguments)
            .env("AGYLINE_HOME", self.0.join("data"))
            .env("AGY_CONFIG_DIR", self.0.join("host"))
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_executable_renders_native_stdin_and_sanitizes_opt_in_capture() {
    let sandbox = Sandbox::new();
    let capture = sandbox.0.join("diagnostic snapshot.json");
    let mut input: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/antigravity.json")).unwrap();
    input["email"] = serde_json::json!("private@example.invalid");
    input["access_token"] = serde_json::json!("never-record-this");
    let output = sandbox.render(
        &input.to_string(),
        &["--capture", capture.to_str().unwrap()],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rendered = String::from_utf8(output.stdout).unwrap();
    assert!(
        rendered.contains("Gemini 4 Neon") && rendered.contains("weekly 82%"),
        "{rendered}"
    );
    assert!(!rendered.contains("private@example.invalid"));
    let snapshot = std::fs::read_to_string(&capture).unwrap();
    assert!(
        !snapshot.contains("private@example.invalid") && !snapshot.contains("never-record-this")
    );
    let parsed = ccometixline::input::Input::parse(&snapshot).unwrap();
    assert!(parsed.is_antigravity());
}

#[cfg(windows)]
#[test]
fn windows_terminal_uses_mocha_rgb_when_color_is_automatic() {
    let sandbox = Sandbox::new();
    for (disable_color, expected_rgb) in [(false, true), (true, false)] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agyline"));
        command
            .args(["preview", "--scenario", "antigravity"])
            .env("AGYLINE_HOME", sandbox.0.join("data"))
            .env("AGY_CONFIG_DIR", sandbox.0.join("host"))
            .env("WT_SESSION", "terminal-test")
            .env_remove("COLORTERM")
            .env_remove("TERM")
            .env_remove("NO_COLOR");
        if disable_color {
            command.env("NO_COLOR", "1");
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        let preview = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            preview.contains("38;2;180;190;254"),
            expected_rgb,
            "{preview:?}"
        );
    }
}

#[test]
fn malformed_input_never_echoes_raw_private_data() {
    let sandbox = Sandbox::new();
    let output = sandbox.render("{broken private@example.invalid access-token", &[]);
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).unwrap();
    assert!(rendered.contains("invalid status line input"));
    assert!(!rendered.contains("private@example.invalid") && !rendered.contains("access-token"));
}

#[test]
fn broken_config_keeps_the_native_status_line_visible() {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(sandbox.0.join("data")).unwrap();
    std::fs::write(sandbox.0.join("data/config.toml"), "palette = [broken").unwrap();
    let output = sandbox.render(include_str!("fixtures/antigravity.json"), &[]);
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).unwrap();
    assert!(rendered.contains("Gemini 4 Neon") && rendered.contains("agyline config"));
}

#[test]
fn every_native_preset_uses_only_confirmed_host_metrics() {
    for preset in ccometixline::presets::PRESETS {
        let configuration = ccometixline::presets::get(preset.name).unwrap();
        for item in configuration
            .lines
            .iter()
            .flat_map(|line| line.left.iter().chain(&line.right))
        {
            assert!(
                ccometixline::metrics::antigravity_supported(&item.config().metric),
                "{}: {}",
                preset.name,
                item.config().metric
            );
        }
    }
    assert_eq!(ccometixline::presets::default_config().lines.len(), 2);
}
