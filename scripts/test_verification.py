"""Regression checks for the opt-in verification helpers, using isolated data."""

from __future__ import annotations

import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

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

    def test_session_requires_native_quota_and_terminal_output(self) -> None:
        native = {"product": "antigravity", "quota": {"dynamic-id": {"remaining_fraction": 0}}}
        for snapshot, quota_visible, expected in [
            (native, False, False), (native, True, True), (None, True, False),
            ({"product": "antigravity", "quota": {}}, True, False),
            ({"product": "other", "quota": native["quota"]}, True, False),
        ]:
            with self.subTest(snapshot=snapshot, quota_visible=quota_visible):
                self.assertEqual(verify_agy_host.has_quota_evidence(
                    installed=False, snapshot=snapshot, quota_visible=quota_visible,
                ), expected)

    def test_persistent_check_needs_output_but_has_no_capture(self) -> None:
        for quota_visible in [True, False]:
            with self.subTest(quota_visible=quota_visible):
                self.assertEqual(verify_agy_host.has_quota_evidence(
                    installed=True, snapshot=None, quota_visible=quota_visible,
                ), quota_visible)


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
