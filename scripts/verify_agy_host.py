"""Opt-in Windows host smoke test; never sends a model prompt or logs raw TUI output.

Requires pywinpty in an isolated environment. Uses a per-session --settings file
instead of editing the user's persistent AGY settings. Captures only agyline's
typed, sanitized diagnostic payload under the ignored local/ directory.
"""

from __future__ import annotations

import argparse
import base64
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time


def has_quota_evidence(*, installed: bool, snapshot: object, quota_visible: bool) -> bool:
    """This smoke test checks a quota-displaying layout, not every valid layout."""
    if not quota_visible:
        return False
    if installed:
        return True  # Persistent mode deliberately does not capture a payload.
    if not isinstance(snapshot, dict) or snapshot.get("product") != "antigravity":
        return False
    buckets = snapshot.get("quota")
    return isinstance(buckets, dict) and bool(buckets)


def reserve_output_directory(requested: Path) -> Path:
    directory = requested.resolve()
    try:
        directory.mkdir(parents=True, exist_ok=False)
    except FileExistsError:
        # Never delete or consume a payload left by another session.
        return Path(tempfile.mkdtemp(prefix="run-", dir=directory))
    return directory


def callback_command(binary: Path, capture_path: Path) -> str:
    """Avoid cmd's leading-quote bug and encode every callback argument."""
    system_directory = ctypes.create_unicode_buffer(32768)
    length = ctypes.windll.kernel32.GetSystemDirectoryW(system_directory, len(system_directory))
    if not 0 < length < len(system_directory):
        raise RuntimeError("cannot resolve the Windows system directory")
    powershell = Path(system_directory.value) / "WindowsPowerShell" / "v1.0" / "powershell.exe"
    unsafe_characters = '\"%!^&|<>()'
    if (
        not powershell.is_file()
        or any(character.isspace() or character in unsafe_characters for character in str(powershell))
    ):
        raise RuntimeError("system PowerShell is unavailable at a safe unquoted absolute path")
    arguments = [str(binary), "--capture", str(capture_path)]
    if any(ord(character) < 32 for argument in arguments for character in argument):
        raise ValueError("callback paths must not contain control characters")
    encoded_binary = base64.b64encode(str(binary).encode("utf-16le")).decode("ascii")
    encoded_capture = base64.b64encode(str(capture_path).encode("utf-16le")).decode("ascii")
    script = (
        "$utf8 = [System.Text.UTF8Encoding]::new($false); "
        "[Console]::InputEncoding = $utf8; [Console]::OutputEncoding = $utf8; "
        "$OutputEncoding = $utf8; "
        f"$executable = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{encoded_binary}')); "
        f"$capture = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{encoded_capture}')); "
        "[Console]::In.ReadToEnd() | & $executable --capture $capture; exit $LASTEXITCODE"
    )
    encoded = base64.b64encode(script.encode("utf-16le")).decode("ascii")
    command = f"{powershell} -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}"
    if len(command) >= 8191:
        raise ValueError("encoded callback exceeds the Windows shell command limit")
    return command


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--launcher", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("local/host-verification"))
    parser.add_argument("--timeout", type=float, default=45)
    parser.add_argument("--installed", action="store_true", help="test persistent integration without per-session overrides")
    arguments = parser.parse_args()
    if os.name != "nt":
        parser.error("this optional host check uses Windows ConPTY")
    from winpty import PtyProcess

    binary = arguments.binary.resolve(strict=True)
    launcher = arguments.launcher.resolve(strict=True)
    output_directory = reserve_output_directory(arguments.output)
    print(f"Diagnostic directory: {output_directory}", file=sys.stderr)
    capture_path = output_directory / "payload.json"
    settings_path = output_directory / "session-settings.json"
    if not arguments.installed:
        settings_path.write_text(json.dumps({
            "statusLine": {
                "type": "command", "command": callback_command(binary, capture_path),
                "enabled": True, "stack_with_default": False,
            }
        }), encoding="utf-8")
    host_settings = Path(os.environ.get("AGY_CONFIG_DIR", str(Path.home() / ".gemini" / "antigravity-cli"))) / "settings.json"

    def settings_digest() -> str | None:
        try:
            return hashlib.sha256(host_settings.read_bytes()).hexdigest()
        except FileNotFoundError:
            return None

    settings_before = settings_digest()

    launch_arguments = [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", str(launcher)]
    if not arguments.installed:
        launch_arguments.extend(["--settings", str(settings_path)])
    process = PtyProcess.spawn(
        launch_arguments,
        cwd=str(Path.cwd()), env=dict(os.environ), dimensions=(40, 132),
    )
    output_chunks: list[str] = []
    write_lock = threading.Lock()

    def drain_terminal() -> None:
        try:
            while process.isalive():
                chunk = process.read(8192)
                if not chunk:
                    continue
                # Keep a bounded in-memory buffer only. It may contain the
                # user's account banner and must never be written or printed.
                output_chunks.append(chunk)
                if len(output_chunks) > 100:
                    del output_chunks[:50]
                if "\x1b[6n" in chunk:
                    with write_lock:
                        process.write("\x1b[1;1R")
        except (EOFError, OSError):
            pass

    threading.Thread(target=drain_terminal, daemon=True).start()
    snapshot = None
    try:
        deadline = time.monotonic() + arguments.timeout
        while time.monotonic() < deadline and process.isalive():
            if not arguments.installed:
                try:
                    candidate = json.loads(capture_path.read_text(encoding="utf-8"))
                    snapshot = candidate if isinstance(candidate, dict) else None
                except (FileNotFoundError, json.JSONDecodeError, PermissionError):
                    pass
            if has_quota_evidence(installed=arguments.installed, snapshot=snapshot,
                                  quota_visible="% left" in "".join(output_chunks)):
                time.sleep(2)
                break
            time.sleep(0.25)

        terminal_output = "".join(output_chunks)
        summary = {
            "verification_mode": "persistent settings" if arguments.installed else "session override",
            "native_payload_received": None if arguments.installed else (snapshot or {}).get("product") == "antigravity",
            "model_prompt_sent": False,
            "screenshot_verified": False,
            "remaining_quota_text_observed": "% left" in terminal_output,
            "callback_error_observed": "invalid status line input" in terminal_output,
        }
        if snapshot is not None:
            summary.update({
                "product": snapshot.get("product"),
                "version": snapshot.get("version"),
                "model": (snapshot.get("model") or {}).get("display_name"),
                "terminal_width": snapshot.get("terminal_width"),
                "quota_bucket_count": len(snapshot.get("quota") or {}),
                "plan_available": bool(snapshot.get("plan_tier")),
            })
    finally:
        if process.isalive():
            with write_lock:
                process.write("\x03")
            time.sleep(0.5)
        if process.isalive():
            with write_lock:
                process.write("/exit\r")
            deadline = time.monotonic() + 4
            while process.isalive() and time.monotonic() < deadline:
                time.sleep(0.1)
        if process.isalive():
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        process.close(force=True)

    summary["persistent_settings_changed"] = settings_before != settings_digest()
    (output_directory / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    verified = has_quota_evidence(installed=arguments.installed, snapshot=snapshot,
                                  quota_visible=summary["remaining_quota_text_observed"])
    return 0 if verified and not summary["persistent_settings_changed"] and not summary["callback_error_observed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
