"""Regression checks for the opt-in verification helpers, using isolated data."""

from __future__ import annotations

import base64
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import verify_agy_host


class HostEvidenceTests(unittest.TestCase):
    def test_repeated_checks_do_not_reuse_or_remove_earlier_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            requested = Path(temporary) / "checks"
            first = verify_agy_host.reserve_output_directory(requested)
            capture = first / "payload.json"
            capture.write_text("first run", encoding="utf-8")
            second = verify_agy_host.reserve_output_directory(requested)
            third = verify_agy_host.reserve_output_directory(requested)
            self.assertEqual(first, requested.resolve())
            self.assertEqual(len({first, second, third}), 3)
            self.assertFalse((second / "payload.json").exists())
            self.assertEqual(capture.read_text(encoding="utf-8"), "first run")

    def test_session_matches_a_rendered_bucket_to_its_own_percentage(self) -> None:
        snapshot = {"product": "antigravity", "quota": {
            "3p-5h": {"remaining_fraction": 1}, "3p-weekly": {"remaining_fraction": 0.98},
            "gemini-argon-wtu-weekly": {"remaining_fraction": 0.375},
            "credits": {"remaining_fraction": 0.82}, "tiny": {"remaining_fraction": 0.001},
            "empty": {"remaining_fraction": 0},
        }}
        for terminal_output, expected in [
            ("ctx 0%\r\n3p      5h 100%  weekly 98%", True),
            ("gemini   argon-wtu-weekly \x1b[32m38%\x1b[0m", True),
            ("credits\x1b[5C82%", True), ("tiny 1%", True), ("empty 0%", True),
            ("3p-weekly 98% left \u00b7 reset 1d0h", True),
            ("", False), ("ctx 0%", False), ("98% left", False),
            ("3p  5h 98%  weekly 100%", False), ("some-credits 82%", False),
            ("3p-weekly 18% left | ctx 98%", False), ("credits\r\n82%", False),
            ("credits\x1b[2;5H82%", False), ("\x1b]0;credits 82%\x07ctx 0%", False),
        ]:
            with self.subTest(terminal_output=terminal_output):
                self.assertEqual(verify_agy_host.has_quota_evidence(
                    installed=False, snapshot=snapshot, terminal_output=terminal_output,
                ), expected)

    def test_session_requires_native_reportable_fraction(self) -> None:
        for snapshot in [
            None, {"product": "other", "quota": {"credits": {"remaining_fraction": 0}}},
            {"product": "antigravity", "quota": {}},
            *({"product": "antigravity", "quota": {"credits": bucket}} for bucket in [
                {"remaining_amount": 0}, {"remaining_fraction": False}, {"remaining_fraction": -1},
                {"remaining_fraction": 0, "disabled": True},
            ]),
            {"product": "antigravity", "quota": {"example-ctx": {"remaining_fraction": 0}}},
        ]:
            with self.subTest(snapshot=snapshot):
                self.assertFalse(verify_agy_host.has_quota_evidence(
                    installed=False, snapshot=snapshot, terminal_output="ctx 0% | credits 0% left",
                ))

    def test_persistent_mode_requires_recognizable_quota_output_without_capture(self) -> None:
        for terminal_output, expected in [
            ("3p\x1b[6C5h \x1b[32m100%\x1b[0m  weekly 98%", True),
            ("credits 82%", True), ("claude-weekly 65% left", True),
            ("pool 65% left \u00b7 reset 1d0h", True), ("weekly 65% left", True),
            ("ctx 0%", False), ("ctx 76% left", False), ("76% left", False),
            ("ctx 76% left \u00b7 reset 1d0h", False),
            ("progress 82%", False), ("credits 182%", False), ("credits\r\n82%", False),
            ("\x1b]0;credits 82%\x1b\\ctx 0%", False), ("", False),
        ]:
            with self.subTest(terminal_output=terminal_output):
                self.assertEqual(verify_agy_host.has_quota_evidence(
                    installed=True, snapshot=None, terminal_output=terminal_output,
                ), expected)

    def test_installed_summary_accepts_compact_stream_without_a_payload(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "diagnostic"
            arguments = ["verify", "--installed", "--binary", sys.executable,
                         "--launcher", sys.executable, "--output", str(output)]
            process = Mock()
            # Replay two chunks synchronously, then report that the simulated host exited.
            chunks = ["synthetic-banner\r\n3p\x1b[", "6C5h 100%  weekly 98%"]
            process.read.side_effect = chunks
            process.isalive.side_effect = lambda: process.read.call_count < len(chunks)
            fake_winpty = SimpleNamespace(PtyProcess=Mock(spawn=Mock(return_value=process)))
            isolated_os = SimpleNamespace(name="nt", environ={"AGY_CONFIG_DIR": temporary})
            console = io.StringIO()
            with (
                patch.dict(sys.modules, winpty=fake_winpty), patch.object(sys, "argv", arguments),
                patch.object(verify_agy_host, "os", isolated_os),
                patch.object(verify_agy_host.threading, "Thread",
                             side_effect=lambda target, daemon: SimpleNamespace(start=target)),
                contextlib.redirect_stdout(console), contextlib.redirect_stderr(console),
            ):
                self.assertEqual(verify_agy_host.main(), 0)
            self.assertEqual([file.name for file in output.iterdir()], ["summary.json"])
            summary = json.loads((output / "summary.json").read_text(encoding="utf-8"))
            self.assertTrue(summary["remaining_quota_text_observed"])
            self.assertIsNone(summary["native_payload_received"])
            self.assertNotIn("synthetic-banner", console.getvalue())


class CallbackCommandTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "Windows host callback contract")
    def test_callback_encodes_special_paths_and_preserves_utf8_streams(self) -> None:
        binary = Path("C:/Users/\u7528\u6237 O'Brien & 100%/agyline.exe")
        capture = Path("C:/diagnostics & echo injected%/payload.json")
        command = verify_agy_host.callback_command(binary, capture)
        flags = " -NoLogo -NoProfile -NonInteractive -EncodedCommand "
        launcher, encoded = command.split(flags, 1)
        self.assertTrue(Path(launcher).is_absolute() and Path(launcher).is_file())
        self.assertNotIn('"', command)
        self.assertNotIn("&", command)
        self.assertNotIn("%", command)
        script = base64.b64decode(encoded, validate=True).decode("utf-16le")
        for argument in [binary, capture]:
            encoded_argument = base64.b64encode(str(argument).encode("utf-16le")).decode("ascii")
            self.assertIn(f"FromBase64String('{encoded_argument}')", script)
            self.assertNotIn(str(argument), script)
        self.assertIn("& $executable --capture $capture", script)
        self.assertIn("[Console]::InputEncoding = $utf8", script)
        self.assertIn("[Console]::OutputEncoding = $utf8", script)
        self.assertIn("[Console]::In.ReadToEnd()", script)
        self.assertTrue(script.endswith("exit $LASTEXITCODE"))

    @unittest.skipUnless(os.name == "nt", "Windows PowerShell quoting contract")
    def test_callback_keeps_unicode_quote_paths_as_data_not_script(self) -> None:
        repository = Path(__file__).resolve().parents[1]
        binary = repository / "target" / "debug" / "agyline.exe"
        if not binary.is_file():
            self.skipTest("build the debug agyline executable first")
        with tempfile.TemporaryDirectory(prefix="agyline-callback-") as temporary:
            directory = Path(temporary)
            capture = directory / "O\u2019Brien; Write-Output AGYLINE_INJECTED; #" / "payload.json"
            capture.parent.mkdir()
            command = verify_agy_host.callback_command(binary, capture)
            launcher, encoded = command.split(" -NoLogo -NoProfile -NonInteractive -EncodedCommand ", 1)
            output = subprocess.run(
                [launcher, "-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
                input=(repository / "tests" / "fixtures" / "antigravity.json").read_bytes(),
                capture_output=True,
                env=dict(os.environ, AGYLINE_HOME=str(directory / "tool"), AGY_CONFIG_DIR=str(directory / "host")),
            )
            self.assertEqual(output.returncode, 0, output.stderr.decode("utf-8", errors="replace"))
            self.assertNotIn(b"AGYLINE_INJECTED", output.stdout)
            self.assertTrue(capture.is_file(), "the exact Unicode capture path must be preserved")
            self.assertIn("Gemini 4 Neon", output.stdout.decode("utf-8"))


class InstallVerificationTests(unittest.TestCase):
    def test_normalized_idempotent_install_does_not_require_another_backup(self) -> None:
        repository = Path(__file__).resolve().parents[1]
        binary = repository / "target" / "debug" / ("agyline.exe" if os.name == "nt" else "agyline")
        if not binary.is_file():
            self.skipTest("build the debug agyline executable first")
        with tempfile.TemporaryDirectory(prefix="agyline-verifier-") as temporary:
            directory = Path(temporary)
            home = directory / "tool"
            host = directory / "host"
            host.mkdir()
            settings = host / "settings.json"
            settings.write_text(json.dumps({"unrelated": "preserve me"}), encoding="utf-8")
            environment = dict(os.environ, AGYLINE_HOME=str(home), AGY_CONFIG_DIR=str(host))
            arguments = [
                sys.executable, str(repository / "scripts" / "verify_local_install.py"),
                "--binary", str(binary), "--apply", "--output", str(directory / "summary.json"),
            ]
            first = subprocess.run(arguments, env=environment, capture_output=True, text=True)
            self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
            normalized = json.loads(settings.read_text(encoding="utf-8"))
            for key in ["type", "enabled", "padding", "stack_with_default"]:
                normalized["statusLine"].pop(key, None)
            settings.write_text(json.dumps(normalized), encoding="utf-8")
            before = settings.read_bytes()
            backups = set(host.glob("settings.json.agyline-bak-*"))
            second = subprocess.run(arguments, env=environment, capture_output=True, text=True)
            self.assertEqual(second.returncode, 0, second.stdout + second.stderr)
            self.assertEqual(settings.read_bytes(), before)
            self.assertEqual(set(host.glob("settings.json.agyline-bak-*")), backups)
            self.assertEqual(json.loads(settings.read_bytes())["unrelated"], "preserve me")


if __name__ == "__main__":
    unittest.main()
