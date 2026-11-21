"""Opt-in Windows release smoke test. Requires pywinpty and a release build."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import time
import tomllib
import unittest


class TerminalSetupTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "Windows terminal smoke test")
    def test_setup_saves_selected_preset_without_connecting_host(self) -> None:
        from winpty import PtyProcess

        binary = Path(__file__).resolve().parents[1] / "target/release/agyline.exe"
        with tempfile.TemporaryDirectory(prefix="agyline-tui-") as temporary:
            directory = Path(temporary)
            config_path = directory / "tool/config.toml"
            settings_path = directory / "host/settings.json"
            environment = dict(os.environ, AGYLINE_HOME=str(config_path.parent),
                               AGY_CONFIG_DIR=str(settings_path.parent))
            process = PtyProcess.spawn([str(binary), "--config"], cwd=temporary,
                                       env=environment, dimensions=(40, 132))
            chunks = []

            def drain_terminal() -> None:
                try:
                    while True:
                        chunks.append(process.read(8192))
                except (EOFError, OSError):
                    pass

            def wait_for(description, condition) -> None:
                deadline = time.monotonic() + 8
                while not condition():
                    if time.monotonic() >= deadline:
                        self.fail(f"Timed out waiting for {description}; exit status={process.exitstatus}, "
                                  f"terminal chunks received={len(chunks)}")
                    time.sleep(0.05)

            def terminal_contains(marker: str) -> bool:
                # ConPTY may position the cursor instead of printing a space.
                words = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", " ", "".join(chunks)).split()
                return marker in " ".join(words)

            reader = threading.Thread(target=drain_terminal, daemon=True)
            reader.start()
            try:
                for marker, keys in [("skip setup", "\r"), ("show as icons", "a"),
                                     ("starting point", "\x1b[B\r"), ("apply and finish", "s")]:
                    wait_for(marker, lambda: terminal_contains(marker))
                    process.write(keys)
                wait_for("saved configuration", config_path.is_file)
                process.write("q")
                wait_for("normal TUI exit", lambda: process.exitstatus is not None)
                self.assertEqual(process.exitstatus, 0)
            finally:
                process.close(force=True)
                reader.join(timeout=2)

            config = tomllib.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual(config["preset"], "minimal")
            self.assertEqual(config["glyphs"], "ascii")
            self.assertFalse(settings_path.exists(), "skipping connection wrote host settings")
            preview = subprocess.run([str(binary), "preview", "--width", "132", "--color", "none"],
                                     env=environment, cwd=temporary, capture_output=True,
                                     encoding="utf-8", check=True, timeout=10).stdout
            self.assertEqual(len(preview.splitlines()), 1)
            self.assertIn("Gemini 4 Neon", preview)
            self.assertIn("82% left", preview)


if __name__ == "__main__":
    unittest.main()
