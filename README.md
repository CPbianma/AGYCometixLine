# agyline

[中文主文档](README.zh.md) | English quick start

A Rust status line for **Antigravity CLI (AGY)**, forked from CCometixLine. It keeps the upstream renderer and full-screen TUI while adapting the input protocol and reversible host integration to AGY.

**Version: 0.1.0. Source/local installation only.** A fork repository URL and npm namespace have not been selected. No published npm package or downloadable fork release is advertised. `@cometix/ccline` belongs to the upstream project and is not agyline.

## Build and connect locally

Requires Rust **1.89 or newer**. In a local checkout of this fork:

```sh
cargo build --release --locked
```

The Cargo package is still named `ccometixline`; the public binary is **`agyline`** (`agyline.exe` on Windows).

Windows PowerShell:

```powershell
.\target\release\agyline.exe preview --scenario antigravity --width 100 --color none
.\target\release\agyline.exe install
.\target\release\agyline.exe status
```

macOS / Linux:

```sh
./target/release/agyline preview --scenario antigravity --width 100 --color none
./target/release/agyline install
./target/release/agyline status
```

Installation copies this executable to `~/.gemini/antigravity-cli/agyline/agyline.exe` on Windows, or the same directory with filename `agyline` on Unix. It then connects AGY's `statusLine` to that stable path; a global PATH entry is not created. Run the stable executable directly to open the TUI in a terminal, or pass `--config`.

`AGYLINE_HOME` overrides the tool directory. `AGY_CONFIG_DIR` separately overrides the host directory containing `settings.json`; it does not move the tool directory. All three integration commands accept `--settings <path>` and `--home <path>`. Reuse the same paths when checking or uninstalling:

```sh
./target/release/agyline uninstall
```

An existing settings file is backed up before changes. Uninstall restores **only the original `statusLine`**, not the entire settings file; unrelated settings are retained. Unknown keys are preserved on installation. If the managed `statusLine` was changed externally, the tool refuses to overwrite it. Uninstall keeps the executable and recovery files rather than deleting them.

AGY may omit documented default fields when saving settings; this alone is not an ownership conflict. Non-default values, commands and unknown keys remain protected. On Windows, paths without spaces run directly; spaced paths use the absolute system PowerShell with separately encoded path data and UTF-8 streams, never a launcher resolved through PATH.

## Native data, not invented windows

- Native quota is a dynamic collection of named buckets: remaining fraction or amount, reset information, and explicit `unknown` / `disabled` states.
- Costs marked `~` are **estimates, not billing records**.
- Unreported fields are hidden or represented as unknown. AGY does not provide a reliable fixed 5-hour / 7-day window, cache TTL, or auto-compaction threshold here; agyline does not invent them.
- No credential-file access, model calls, or `agy.exe` patching is required. User-configured custom command metrics run the commands you explicitly choose.

The default `cometix` preset uses **Compact Mocha**: model, directory, Git (when available), context and subscription on the first logical line, then native quota grouped by ID prefix. Percentages mean **remaining**. Healthy buckets have no bar or countdown; low buckets show a reset when reported. Choose `budget` for full IDs, bars and reset details. Existing configuration files are not migrated: to switch, apply **Cometix** in the TUI Presets page and save with Ctrl+S. The other presets are `minimal`, `dashboard`, `session`, `focus`, `zen`, and `powerline`. Four skins and 13 palettes are retained.

TUI: arrows navigate; left/right switch focused tabs, down/Enter enters a page, and up/Esc returns to the tabs when the page yields focus. Tab switches controls within supported pages. **Ctrl+S** saves, **q** requests exit, and **?** opens help. Plain `s` is not a global save shortcut. See the [Chinese guide](README.zh.md) for configuration and recovery details.

## Preview and validation boundaries

```sh
agyline preview --scenario antigravity --width 100 --color none
agyline preview --scenario agy-low-quota --width 100 --color none
agyline preview --scenario agy-startup --width 100 --color none
agyline preview --list
agyline preview --all
```

These examples assume the stable executable is on your PATH; otherwise use its full path. Previews use **sample input**, not a live AGY session. No upstream screenshot is presented as a screenshot of this fork.

For automated checks:

```sh
cargo test --locked --all-targets
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
python -m unittest discover -s scripts -p test_verification.py -v
```

CI defines Windows, macOS and Linux tests. `release.yml` is manual-only and builds workflow artifacts; it does **not** create a GitHub Release or publish to npm. These configurations and sample previews are not evidence of a completed remote workflow run.

**Compact Mocha verification:** Windows passed 255 Rust tests, eight verifier regressions and one release TUI interaction test. Real AGY **1.5.14** produced quota text with both session and persistent settings. The installed binary matches the release build. See the [compact rollout record](docs/superpowers/plans/2026-11-21-compact-quota.md) and the [earlier baseline and reproduction commands](docs/verification/2026-11-21-agyline.md). Terminal checks do not certify desktop fonts or repainting.

**Local verification on November 20, 2026:** Windows passed 252 Rust tests, 3 verifier regressions, formatting, strict Clippy and a release build. Antigravity CLI 1.5.13 supplied a native payload containing five quota buckets at 132 columns. Both session overrides and the persistent installation produced remaining-quota text without sending a model prompt or modifying persistent settings during startup checks. Installation preserved unrelated settings and verified the stable executable against the build.

**Boundaries:** desktop screenshot/font/repaint inspection remains unverified because the screenshot helper was unavailable. Linux/macOS and remote CI were not run in this session. Restore and conflict handling were verified in isolated tests, not by uninstalling the user's active integration. See the [verification record](docs/verification/2026-11-20-agyline.md). Optional `scripts/verify_agy_host.py` requires Windows and an isolated `pywinpty` environment; it keeps diagnostic files under ignored `local/` and does not record raw terminal banners. Do not publish local captures or backups.

## Origin and license status

Upstream: [Haleclipse/CCometixLine](https://github.com/Haleclipse/CCometixLine), pinned at [`1749ff85b52d8d10e67e0a04d247ceab768f380f`](https://github.com/Haleclipse/CCometixLine/tree/1749ff85b52d8d10e67e0a04d247ceab768f380f).

Upstream README and Cargo metadata declare MIT, but no standalone `LICENSE` was present in the pinned tree. This fork records that gap rather than inventing a license text or copyright notice. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for attribution, retained assets, and the outstanding licensing check before redistribution.
