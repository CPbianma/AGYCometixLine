# agyline: native Antigravity status line

## Approved scope

The user approved an Antigravity CLI status line embedded in the existing agy
prompt footer, with CCometixLine's appearance and terminal configurator. On
2026-11-20 the user authorized implementation, local Git initialization, and
frequent commits. No remote publication is authorized.

## Provenance

Start from Haleclipse/CCometixLine commit
`1749ff85b52d8d10e67e0a04d247ceab768f380f` (package version 2.0.1). Preserve its
Git history and upstream remote. The manifest and README declare MIT; the
pinned tree has no standalone LICENSE file. Retain attribution and record this
fact rather than inventing the original copyright notice.

## Host contract

Authoritative reference: https://antigravity.google/docs/cli/statusline.md

`~/.gemini/antigravity-cli/settings.json` accepts an object named `statusLine`
with `type`, `command`, `padding` (vertical blank lines), `enabled`, and
`stack_with_default`. Commands run through cmd /c on Windows and sh -c on Unix.
The host supplies JSON on stdin and renders multiline ANSI stdout. Do not
patch agy.exe, intercept terminal rendering, or read authentication tokens.

Normalize native fields: conversation_id/title, model.effort,
terminal_width, agent_state, plan_tier, quota, cost.total_usd and cost.estimated.
Common workspace/context fields retain compatibility with the upstream model.
Never enforce Claude Code version gates on Antigravity version 1.x.

## Architecture

Keep the Rust rendering, skin, palette, widget, wrapping, preset and TUI core.
Add a focused Antigravity payload/metrics adapter. Host settings, paths, and
integration UI become Antigravity-specific. Internal crate identifiers may
remain unchanged where renaming would add risk; the public executable is
`agyline`. Default data directory: `~/.gemini/antigravity-cli/agyline`, override
with `AGYLINE_HOME`; host config root override: `AGY_CONFIG_DIR`.

Use native terminal_width for each render, reserving only confirmed footer
space. Do not interpret statusLine.padding as horizontal padding. Do not read
Claude settings or environment overrides. Do not install Claude companion
mods or write subagentStatusLine. Do not query upstream ccline releases as
agyline updates. Keep unsupported metrics out of adapted presets and clearly
unavailable in the component picker.

## Metrics and presentation

Default two-row classic layout: model/effort, directory, Git, context; then
subscription tier and dynamic quota buckets with remaining amount/percentage
and reset countdown. Reuse upstream palettes and skins, including powerline,
pill, bracket, plain and Unicode/ASCII fallbacks. Preserve customization,
preview, and TUI layout/style/preset editing.

Quota bucket IDs are dynamic; do not assume Claude's five-hour/seven-day
windows. A remaining_fraction in [0,1] becomes a remaining percentage; low
remaining quota is a warning, zero is exhausted. remaining_amount without a
known limit must not become a fabricated percentage. Malformed, unknown,
missing and disabled values remain distinguishable from zero. Display reset
times only when trustworthy; expired reset times must not claim refilled
quota. Context percentage follows the host payload, not the upstream 200k
fallback or Claude auto-compaction calculation. Costs are estimates, never
represented as subscription billing.

## Safe installation and privacy

Provide preview, install/status/uninstall operations and equivalent TUI
integration. Use a stable installed binary outside target/. Preserve unrelated
JSON settings and unknown statusLine keys. Back up before writes, detect
concurrent changes, quote executable paths correctly, and restore the prior
statusLine on uninstall without overwriting subsequent user edits. Invalid
settings fail closed. Do not commit email addresses, raw transcripts or local
payload captures. Default display excludes email; runtime replay persistence
must redact sensitive payload fields.

## Acceptance and verification

Tests cover native payloads, missing and malformed optional fields, context
semantics, quota polarity/zero/amount/dynamic buckets, narrow widths, all skins,
isolated paths, settings preservation/backups/restore/conflicts and executable
paths with spaces. Run upstream tests, fmt, clippy, build release and smoke
tests. Use sanitized fixtures. Real local agy payload and rendered UI must be
verified separately from fixture previews. Do not send a paid model prompt
merely to test a status line. If the host cannot be exercised in this session,
report that boundary explicitly and provide a reproducible diagnostic.

## Git policy

Commit coherent stages after checks: approved design/plan, host protocol and
metrics, integration/configurator, packaging/docs, final fixes. Keep generated
binaries and private local captures ignored. Do not modify git identity or
push without authorization.
