"""Explicitly install a local build and verify backups without logging settings."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--apply", action="store_true", help="required to modify host settings")
    parser.add_argument("--output", type=Path, default=Path("local/install-verification.json"))
    arguments = parser.parse_args()
    if not arguments.apply:
        parser.error("pass --apply to authorize this persistent installation")
    binary = arguments.binary.resolve(strict=True)
    host_directory = Path(os.environ.get("AGY_CONFIG_DIR") or Path.home() / ".gemini" / "antigravity-cli")
    tool_directory = Path(os.environ.get("AGYLINE_HOME") or Path.home() / ".gemini" / "antigravity-cli" / "agyline")
    settings_path = host_directory / "settings.json"
    original_bytes = settings_path.read_bytes() if settings_path.exists() else None
    original = json.loads(original_bytes) if original_bytes is not None else {}
    original_unrelated = {key: value for key, value in original.items() if key != "statusLine"}

    subprocess.run([str(binary), "install"], check=True)
    updated_bytes = settings_path.read_bytes()
    updated = json.loads(updated_bytes)
    unrelated = {key: value for key, value in updated.items() if key != "statusLine"}
    assert unrelated == original_unrelated, "unrelated host settings changed"
    settings_rewritten = updated_bytes != original_bytes
    if original_bytes is not None and settings_rewritten:
        backups = settings_path.parent.glob(settings_path.name + ".agyline-bak-*")
        backup_matches = any(path.read_bytes() == original_bytes for path in backups)
        assert backup_matches, "a byte-identical settings backup was not found"
    else:
        backup_matches = None
    stable_binary = tool_directory / ("agyline.exe" if os.name == "nt" else "agyline")
    assert stable_binary.read_bytes() == binary.read_bytes(), "installed binary differs from verified build"
    status = subprocess.run(
        [str(stable_binary), "status"], check=True, capture_output=True, encoding="utf-8",
    )
    # Use the installer's ownership check, including AGY's omitted defaults.
    assert status.stdout.startswith("agyline: installed\n"), "installed status line has an ownership conflict"
    print(status.stdout, end="")
    summary = {
        "installed": True,
        "settings_rewritten": settings_rewritten,
        "unrelated_host_settings_preserved": unrelated == original_unrelated,
        "original_backup_matches_bytes": backup_matches,
        "stable_binary_matches_verified_build": True,
        "original_statusline_present": "statusLine" in original,
        "model_prompt_sent": False,
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
