use ccometixline::cc_settings::Settings;
use ccometixline::integration::Installation;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
    settings: PathBuf,
    home: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agyline integration {} {} 中文",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self {
            settings: root.join("host settings.json"),
            home: root.join("stable home"),
            root,
        }
    }

    fn run(&self, action: &str) -> Output {
        let executable = option_env!("CARGO_BIN_EXE_agyline")
            .or(option_env!("CARGO_BIN_EXE_ccometixline"))
            .expect("a cargo binary");
        Command::new(executable)
            .arg(action)
            .arg("--settings")
            .arg(&self.settings)
            .arg("--home")
            .arg(&self.home)
            .env("AGYLINE_HOME", &self.home)
            .env("AGY_CONFIG_DIR", self.root.join("unused host"))
            .output()
            .unwrap()
    }

    fn write(&self, value: Value) {
        std::fs::write(&self.settings, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }

    fn read(&self) -> Value {
        read_json(&self.settings)
    }

    fn record(&self) -> PathBuf {
        self.home.join("installation.json")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn assert_success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn hold_file_lock(path: &Path) -> File {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    file.try_lock().unwrap();
    file
}

#[test]
fn install_is_stable_idempotent_and_restores_only_statusline() {
    let sandbox = Sandbox::new();
    let original =
        json!({"type":"command", "command":"old tool", "padding":2, "futureKey":{"keep":true}});
    sandbox.write(json!({"statusLine":original, "privateRoot":"not in installation record"}));
    assert_success(sandbox.run("install"));
    let installed = sandbox.read();
    let command = installed["statusLine"]["command"].as_str().unwrap();
    let executable = sandbox.home.join(if cfg!(windows) {
        "agyline.exe"
    } else {
        "agyline"
    });
    assert!(executable.is_file());
    assert!(command.contains(executable.to_str().unwrap()));
    assert!(!command.contains("target"));
    assert_eq!(installed["statusLine"]["type"], "command");
    assert_eq!(installed["statusLine"]["enabled"], true);
    assert_eq!(installed["statusLine"]["futureKey"], original["futureKey"]);
    for forbidden in [
        "refreshInterval",
        "hideVimModeIndicator",
        "subagentStatusLine",
    ] {
        assert!(installed["statusLine"].get(forbidden).is_none());
    }
    let record = std::fs::read(sandbox.record()).unwrap();
    assert!(!String::from_utf8_lossy(&record).contains("privateRoot"));
    assert_success(sandbox.run("install"));
    assert_eq!(record, std::fs::read(sandbox.record()).unwrap());
    let status = assert_success(sandbox.run("status"));
    assert!(
        status.contains("installed") && status.contains("installation.json"),
        "{status}"
    );
    let mut changed = sandbox.read();
    changed["newRoot"] = json!("keep later changes");
    sandbox.write(changed);
    assert_success(sandbox.run("uninstall"));
    assert_eq!(sandbox.read()["statusLine"], original);
    assert_eq!(sandbox.read()["newRoot"], "keep later changes");
    assert_success(sandbox.run("uninstall"));
}

#[test]
fn absent_statusline_is_removed_and_a_changed_one_is_never_overwritten() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"theme":"before"}));
    assert_success(sandbox.run("install"));
    let installed = sandbox.read()["statusLine"].clone();
    sandbox.write(json!({"statusLine":{"command":"someone else"}, "theme":"after"}));
    assert!(!sandbox.run("uninstall").status.success());
    assert_eq!(sandbox.read()["statusLine"]["command"], "someone else");
    assert!(!sandbox.run("install").status.success());
    sandbox.write(json!({"statusLine":installed, "theme":"after"}));
    assert_success(sandbox.run("uninstall"));
    assert!(sandbox.read().get("statusLine").is_none());
    assert_eq!(sandbox.read()["theme"], "after");
}

#[test]
fn malformed_settings_are_refused_without_writes() {
    for content in [
        " ",
        "{",
        "[]",
        r#"{"statusLine":false}"#,
        r#"{"statusLine":{"type":"unsupported"}}"#,
        r#"{"statusLine":{"padding":"wide"}}"#,
    ] {
        let sandbox = Sandbox::new();
        std::fs::write(&sandbox.settings, content).unwrap();
        assert!(
            !sandbox.run("install").status.success(),
            "accepted {content}"
        );
        assert_eq!(std::fs::read_to_string(&sandbox.settings).unwrap(), content);
        assert!(!sandbox.record().exists());
    }
}

#[test]
fn existing_installation_lock_refuses_changes() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    let lock_path = sandbox.home.join("installation.lock");
    std::fs::write(&lock_path, "do not truncate before obtaining the lock").unwrap();
    let lock = hold_file_lock(&lock_path);
    assert!(!sandbox.run("install").status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    drop(lock);
    assert_eq!(
        std::fs::read_to_string(&lock_path).unwrap(),
        "do not truncate before obtaining the lock"
    );
    assert_success(sandbox.run("install"));
    assert!(lock_path.exists());
}

#[test]
fn existing_copy_staging_link_never_truncates_an_unrelated_file() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep": true}));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    let victim = sandbox.root.join("unrelated-file.txt");
    std::fs::write(&victim, "must remain unchanged").unwrap();
    let executable = sandbox.home.join(if cfg!(windows) {
        "agyline.exe"
    } else {
        "agyline"
    });
    let temporary = executable.with_extension(format!("copy-{}", std::process::id()));
    std::fs::hard_link(&victim, &temporary).unwrap();
    let installation = Installation::new(sandbox.settings.clone(), sandbox.home.clone());
    assert!(installation.install().is_err());
    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        "must remain unchanged"
    );
    assert_eq!(sandbox.read(), json!({"keep": true}));
    assert!(!sandbox.record().exists());
}

#[test]
fn atomic_writer_never_follows_an_existing_temporary_hard_link() {
    let sandbox = Sandbox::new();
    let target = sandbox.root.join("atomic.json");
    std::fs::write(&target, "before").unwrap();
    let victim = sandbox.root.join("unrelated-file.txt");
    std::fs::write(&victim, "must remain unchanged").unwrap();
    let temporary = target.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::hard_link(&victim, &temporary).unwrap();
    assert!(ccometixline::paths::write_atomic(&target, "after").is_err());
    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        "must remain unchanged"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "before");
}

#[test]
fn installed_command_runs_from_a_unicode_path_with_spaces() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({}));
    assert_success(sandbox.run("install"));
    let command = sandbox.read()["statusLine"]["command"]
        .as_str()
        .unwrap()
        .to_string();
    #[cfg(windows)]
    let mut process = {
        use std::os::windows::process::CommandExt;
        let mut process = Command::new("cmd.exe");
        // cmd.exe parses a command string, not MSVC-escaped argv. Match
        // the host shell's outer /s /c quotes without adding backslashes.
        process
            .args(["/d", "/s", "/c"])
            .raw_arg(format!("\"{command} --version\""));
        process
    };
    #[cfg(not(windows))]
    let mut process = {
        let mut process = Command::new("sh");
        process.args(["-c", &format!("{command} --version")]);
        process
    };
    let output = process.output().unwrap();
    let version = assert_success(output);
    assert!(version.contains("agyline 0.1.0"), "{version}");
}

#[test]
fn reviewed_install_and_uninstall_refuse_later_disk_edits() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":1}));
    let installation = Installation::new(sandbox.settings.clone(), sandbox.home.clone());
    let (base, draft) = installation.preview_install().unwrap();
    sandbox.write(json!({"keep":2}));
    assert!(installation.apply_install(&base, &draft).is_err());
    assert!(!sandbox.record().exists());
    assert_eq!(sandbox.read()["keep"], 2);
    assert_success(sandbox.run("install"));
    let (base, _) = installation.preview_uninstall().unwrap();
    let mut changed = sandbox.read();
    changed["keep"] = json!(3);
    sandbox.write(changed.clone());
    assert!(installation.apply_uninstall(&base).is_err());
    assert_eq!(sandbox.read(), changed);
    assert_success(sandbox.run("uninstall"));
    assert_eq!(sandbox.read(), json!({"keep":3}));
}

#[test]
fn backups_preserve_original_bytes_and_later_backups_do_not_replace_them() {
    let sandbox = Sandbox::new();
    let original =
        "{\"privateRoot\":\"preserve exactly\",\"statusLine\":{\"command\":\"old tool\"}}";
    std::fs::write(&sandbox.settings, original).unwrap();
    assert_success(sandbox.run("install"));
    let backups = || {
        std::fs::read_dir(&sandbox.root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(".agyline-bak-")
            })
            .collect::<Vec<_>>()
    };
    let first_backup = backups();
    assert_eq!(first_backup.len(), 1);
    assert_eq!(std::fs::read_to_string(&first_backup[0]).unwrap(), original);
    assert_success(sandbox.run("uninstall"));
    assert_eq!(backups().len(), 2);
    assert_eq!(std::fs::read_to_string(&first_backup[0]).unwrap(), original);
}

#[test]
fn existing_unknown_host_keys_survive_installation_and_options_are_agy_only() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({
        "statusLine":{"command":"previous", "refreshInterval":17,"hideVimModeIndicator":true,"future":9},
        "subagentStatusLine":{"command":"leave alone"}
    }));
    assert_success(sandbox.run("install"));
    let written = sandbox.read();
    assert_eq!(written["statusLine"]["refreshInterval"], 17);
    assert_eq!(written["statusLine"]["hideVimModeIndicator"], true);
    assert_eq!(written["statusLine"]["future"], 9);
    assert_eq!(written["subagentStatusLine"]["command"], "leave alone");
}

#[test]
fn settings_can_be_missing_and_stable_executable_can_reinstall_itself() {
    let sandbox = Sandbox::new();
    assert_success(sandbox.run("install"));
    let before = std::fs::read(sandbox.record()).unwrap();
    let executable = sandbox.home.join(if cfg!(windows) {
        "agyline.exe"
    } else {
        "agyline"
    });
    let output = Command::new(executable)
        .arg("install")
        .arg("--settings")
        .arg(&sandbox.settings)
        .arg("--home")
        .arg(&sandbox.home)
        .env("AGYLINE_HOME", &sandbox.home)
        .env("AGY_CONFIG_DIR", &sandbox.root)
        .output()
        .unwrap();
    assert_success(output);
    assert_eq!(std::fs::read(sandbox.record()).unwrap(), before);
    assert_success(sandbox.run("uninstall"));
    assert_eq!(sandbox.read(), json!({}));
}

#[test]
fn host_padding_is_vertical_and_auto_compaction_thresholds_are_unknown() {
    let sandbox = Sandbox::new();
    sandbox.write(
        json!({"statusLine":{"padding":8}, "autoCompactEnabled":true, "autoCompactWindow":200000}),
    );
    let settings = ccometixline::env::ClaudeSettings::load_path(&sandbox.settings);
    assert_eq!(settings.statusline_vertical_padding(), 8);
    assert_eq!(settings.statusline_padding(), 0);
    assert!(!settings.auto_compact().enabled);
    assert_eq!(settings.auto_compact().window, None);
    assert_eq!(settings.auto_compact().pct, None);
}

#[test]
fn host_settings_lock_prevents_a_second_tool_home_from_writing() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    let mut lock = sandbox.settings.as_os_str().to_os_string();
    lock.push(".agyline-lock");
    let lock_path = PathBuf::from(lock);
    let lock = hold_file_lock(&lock_path);
    assert!(!sandbox.run("install").status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    drop(lock);
    assert_success(sandbox.run("install"));
    assert!(lock_path.exists());
}

#[test]
fn inactive_lock_files_without_a_live_owner_do_not_block_installation() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    let home_lock = sandbox.home.join("installation.lock");
    let mut host_lock = sandbox.settings.as_os_str().to_os_string();
    host_lock.push(".agyline-lock");
    let host_lock = PathBuf::from(host_lock);
    for path in [&home_lock, &host_lock] {
        std::fs::write(path, "pid=999999999 (previous process no longer exists)").unwrap();
    }
    assert_success(sandbox.run("install"));
    assert_eq!(sandbox.read()["keep"], true);
    assert!(home_lock.is_file() && host_lock.is_file());
    assert_success(sandbox.run("uninstall"));
    assert_eq!(sandbox.read(), json!({"keep":true}));
}

struct LockProcess(Child);

impl LockProcess {
    fn start(sandbox: &Sandbox, path: &Path) -> Self {
        let ready_path = sandbox.root.join("lock-owner-ready");
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "lock_holder_subprocess", "--nocapture"])
            .env("AGYLINE_TEST_LOCK_PATH", path)
            .env("AGYLINE_TEST_READY_PATH", &ready_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut process = Self(child);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready_path.is_file() {
            assert!(
                process.0.try_wait().unwrap().is_none(),
                "lock owner exited before acquiring the lock"
            );
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the lock owner"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        process
    }

    fn terminate(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

impl Drop for LockProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn lock_holder_subprocess() {
    let Some(path) = std::env::var_os("AGYLINE_TEST_LOCK_PATH") else {
        return;
    };
    let _lock = hold_file_lock(Path::new(&path));
    std::fs::write(
        std::env::var_os("AGYLINE_TEST_READY_PATH").unwrap(),
        "ready",
    )
    .unwrap();
    // The parent can close stdin for a normal exit or kill this process to
    // verify automatic OS-lock release without Rust destructors running.
    let mut buffer = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer).unwrap();
}

#[test]
fn abnormal_process_exit_releases_the_file_lock_for_retry() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    let lock_path = sandbox.home.join("installation.lock");
    let mut owner = LockProcess::start(&sandbox, &lock_path);
    assert!(!sandbox.run("install").status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    owner.terminate();
    assert!(lock_path.exists());
    assert_success(sandbox.run("install"));
    assert!(lock_path.exists());
}

fn simulate_active_record_with_original_settings(
    sandbox: &Sandbox,
    original: Option<Value>,
) -> Value {
    let mut root = json!({"unrelated":"before"});
    if let Some(original) = original {
        root["statusLine"] = original;
    }
    sandbox.write(root.clone());
    assert_success(sandbox.run("install"));
    let record = read_json(&sandbox.record());
    // This is the durable boundary common to a killed write-ahead install
    // and an uninstall whose settings commit preceded its record update.
    root["unrelated"] = json!("changed after interruption");
    sandbox.write(root);
    record
}

#[test]
fn interrupted_write_ahead_install_retries_without_losing_the_original_restore_point() {
    for original in [
        None,
        Some(json!({})),
        Some(json!({"type":"command", "command":"original tool", "future":true})),
    ] {
        let sandbox = Sandbox::new();
        let record = simulate_active_record_with_original_settings(&sandbox, original);
        let status = assert_success(sandbox.run("status"));
        assert!(status.contains("recovery pending"), "{status}");
        assert_success(sandbox.run("install"));
        let retried = read_json(&sandbox.record());
        assert_eq!(
            retried, record,
            "retry must retain the original recovery point and identity"
        );
        assert_eq!(
            sandbox.read()["statusLine"],
            record["installed_status_line"]
        );
        assert_eq!(sandbox.read()["unrelated"], "changed after interruption");
        assert_success(sandbox.run("uninstall"));
        let restored = sandbox.read();
        assert_eq!(
            restored.get("statusLine"),
            record["original_present"]
                .as_bool()
                .unwrap()
                .then_some(&record["original_status_line"])
        );
        assert_eq!(restored["unrelated"], "changed after interruption");
    }
}

#[test]
fn uninstall_finishes_only_the_record_when_settings_have_already_been_restored() {
    for original in [
        None,
        Some(json!({})),
        Some(json!({"command":"original tool", "future":true})),
    ] {
        let sandbox = Sandbox::new();
        let record = simulate_active_record_with_original_settings(&sandbox, original);
        let restored_bytes = std::fs::read(&sandbox.settings).unwrap();
        assert_success(sandbox.run("uninstall"));
        assert_eq!(
            std::fs::read(&sandbox.settings).unwrap(),
            restored_bytes,
            "already restored settings must not be rewritten"
        );
        let completed = read_json(&sandbox.record());
        let mut expected = record;
        expected["active"] = json!(false);
        assert_eq!(completed, expected);
        assert_success(sandbox.run("uninstall"));
        assert_success(sandbox.run("install"));
        assert_success(sandbox.run("uninstall"));
        assert_eq!(
            sandbox.read(),
            serde_json::from_slice::<Value>(&restored_bytes).unwrap()
        );
    }
}

#[test]
fn recovery_can_finish_uninstall_without_creating_an_originally_absent_settings_file() {
    let sandbox = Sandbox::new();
    let record = simulate_active_record_with_original_settings(&sandbox, None);
    std::fs::remove_file(&sandbox.settings).unwrap();
    assert_success(sandbox.run("uninstall"));
    assert!(!sandbox.settings.exists());
    let mut expected = record;
    expected["active"] = json!(false);
    assert_eq!(read_json(&sandbox.record()), expected);
}

#[cfg(windows)]
#[test]
fn uninstall_can_retry_after_settings_restore_succeeds_but_record_update_fails() {
    let sandbox = Sandbox::new();
    let original = json!({"command":"original tool", "future":true});
    sandbox.write(json!({"statusLine":original, "unrelated":"keep me"}));
    assert_success(sandbox.run("install"));
    let record = read_json(&sandbox.record());
    let original_permissions = std::fs::metadata(sandbox.record()).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(sandbox.record(), permissions.clone()).unwrap();
    let failed = sandbox.run("uninstall");
    std::fs::set_permissions(sandbox.record(), original_permissions).unwrap();
    assert!(!failed.status.success());
    assert_eq!(
        sandbox.read(),
        json!({"statusLine":original, "unrelated":"keep me"})
    );
    assert_eq!(read_json(&sandbox.record()), record);
    let restored_bytes = std::fs::read(&sandbox.settings).unwrap();
    assert_success(sandbox.run("uninstall"));
    assert_eq!(std::fs::read(&sandbox.settings).unwrap(), restored_bytes);
    let mut expected = record;
    expected["active"] = json!(false);
    assert_eq!(read_json(&sandbox.record()), expected);
}

#[test]
fn recovery_never_confuses_absent_statusline_with_a_present_empty_object() {
    for original in [None, Some(json!({}))] {
        let sandbox = Sandbox::new();
        let record = simulate_active_record_with_original_settings(&sandbox, original);
        let mut external = sandbox.read();
        if record["original_present"] == true {
            external.as_object_mut().unwrap().remove("statusLine");
        } else {
            external["statusLine"] = json!({});
        }
        sandbox.write(external.clone());
        assert!(!sandbox.run("install").status.success());
        assert!(!sandbox.run("uninstall").status.success());
        assert_eq!(sandbox.read(), external);
        assert_eq!(read_json(&sandbox.record()), record);
    }
}

#[test]
fn malformed_or_foreign_records_are_not_silently_replaced() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    std::fs::write(sandbox.record(), "{corrupt record}").unwrap();
    assert!(!sandbox.run("install").status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    assert_eq!(
        std::fs::read_to_string(sandbox.record()).unwrap(),
        "{corrupt record}"
    );
    assert!(Settings::load(&sandbox.settings).is_ok());
}

#[cfg(windows)]
#[test]
fn windows_expansion_symbols_are_rejected_before_installation() {
    let mut sandbox = Sandbox::new();
    sandbox.home = sandbox.root.join("unsafe %TEMP% !name! & command");
    sandbox.write(json!({"keep":true}));
    assert!(!sandbox.run("install").status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    assert!(!sandbox.record().exists());
}

#[test]
fn environment_overrides_select_agy_paths_without_changing_other_settings() {
    let sandbox = Sandbox::new();
    let host = sandbox.root.join("host");
    std::fs::create_dir_all(&host).unwrap();
    let host_settings = host.join("settings.json");
    std::fs::write(&host_settings, "{\"keep\":true}").unwrap();
    let executable = env!("CARGO_BIN_EXE_agyline");
    let output = Command::new(executable)
        .arg("install")
        .env("AGYLINE_HOME", &sandbox.home)
        .env("AGY_CONFIG_DIR", &host)
        .output()
        .unwrap();
    assert_success(output);
    assert_eq!(read_json(&host_settings)["keep"], true);
    assert!(
        read_json(&host_settings)["statusLine"]["command"]
            .as_str()
            .unwrap()
            .contains("agyline")
    );
    assert!(!sandbox.settings.exists());
}

fn render_in_sandbox(sandbox: &Sandbox, payload: &[u8]) -> Output {
    use std::io::Write;
    use std::process::Stdio;
    let executable = env!("CARGO_BIN_EXE_agyline");
    let mut child = Command::new(executable)
        .current_dir(&sandbox.root)
        .env("AGYLINE_HOME", &sandbox.home)
        .env("AGY_CONFIG_DIR", &sandbox.root)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(payload).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stdin_errors_do_not_disclose_payload_and_invalid_config_still_renders() {
    let sandbox = Sandbox::new();
    let output = assert_success(render_in_sandbox(&sandbox, b"{ private secret payload"));
    assert!(!output.contains("private") && !output.contains("secret"));
    std::fs::create_dir_all(&sandbox.home).unwrap();
    std::fs::write(sandbox.home.join("config.toml"), "invalid = =").unwrap();
    let payload = json!({
        "product":"antigravity", "version":"0.4.7", "terminal_width":160,
        "model":{"id":"gemini-test", "display_name":"AGY fallback model"},
        "tasks":[]
    });
    let output = assert_success(render_in_sandbox(&sandbox, payload.to_string().as_bytes()));
    assert!(
        output.contains("AGY fallback model") && output.contains("agyline config"),
        "{output}"
    );
}

#[cfg(windows)]
#[test]
fn a_failed_settings_write_does_not_poison_the_restoration_record() {
    let sandbox = Sandbox::new();
    sandbox.write(json!({"keep":true}));
    let original_permissions = std::fs::metadata(&sandbox.settings).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&sandbox.settings, permissions.clone()).unwrap();
    let failed = sandbox.run("install");
    std::fs::set_permissions(&sandbox.settings, original_permissions).unwrap();
    assert!(!failed.status.success());
    assert_eq!(sandbox.read(), json!({"keep":true}));
    assert!(!sandbox.record().exists());
    assert_success(sandbox.run("install"));
}
