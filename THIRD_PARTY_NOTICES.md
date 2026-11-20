# Third-party notices / 第三方来源说明

## CCometixLine

- 上游项目 / Upstream: [Haleclipse/CCometixLine](https://github.com/Haleclipse/CCometixLine)
- 固定基线 / Pinned revision: [`1749ff85b52d8d10e67e0a04d247ceab768f380f`](https://github.com/Haleclipse/CCometixLine/tree/1749ff85b52d8d10e67e0a04d247ceab768f380f)
- 上游 `Cargo.toml` 中声明的作者 / Author listed in upstream Cargo metadata: `Haleclipse`. This is attribution to upstream metadata, not a statement identifying the author of this fork.
- 上游声明的许可证 / Declared upstream license: **MIT**, as stated in the pinned README and `Cargo.toml`.

agyline derives from the Rust implementation, composable renderer, full-screen TUI, configuration/model-registry infrastructure, presets, palettes, glyph support, tests and other retained files of that revision. This fork adapts the host protocol and settings integration for Antigravity CLI, uses the public executable name `agyline`, and disables the inherited npm distribution path. Retained internal identifiers such as the Cargo package name `ccometixline` do not make agyline an upstream release.

agyline 基于上述固定版本的 Rust 实现、渲染器、TUI、配置与模型登记基础设施、预设、配色、字形支持、测试及其他保留文件进行修改。这里注明这些来源，不声称获得上游背书，也不将上游 npm 包或 Releases 作为本 fork 的分发来源。

### License-file gap / 许可证文件缺口

The pinned upstream root tree does **not** contain a standalone `LICENSE` file, although its README links to `LICENSE` and Cargo declares MIT. This notice records that discrepancy; it does not reconstruct a missing license, invent a copyright holder or year, or replace any required license text. The fork's retained MIT metadata is a declaration, not evidence that the complete upstream license notice has been recovered.

固定基线的上游根目录未附独立 `LICENSE` 文件，尽管 README 引用了该文件、Cargo 声明 MIT。本文件如实记录缺口，不补写未经核实的作者身份、版权年份或授权正文。**对外再分发前仍需向上游核实并取得适用的完整许可证与版权声明，保留其原文。** 当前不把许可证材料视为已核实齐全。

Verification references:

- [Pinned Cargo metadata](https://github.com/Haleclipse/CCometixLine/blob/1749ff85b52d8d10e67e0a04d247ceab768f380f/Cargo.toml)
- [Pinned README](https://github.com/Haleclipse/CCometixLine/blob/1749ff85b52d8d10e67e0a04d247ceab768f380f/README.md)
- [Pinned root tree via GitHub API](https://api.github.com/repos/Haleclipse/CCometixLine/git/trees/1749ff85b52d8d10e67e0a04d247ceab768f380f)

### Retained images and visual resources / 保留图片与视觉资源

Existing files under `assets/`, including `hero.webp`, `presets.webp`, `tui-layout.webp` and `tui-icons.webp`, originate in the upstream tree. They illustrate upstream CCometixLine, not verified output from agyline in Antigravity CLI. The fork READMEs do not embed them as fork screenshots. No separate asset license or license for bundled glyph data is asserted here without verification.

`assets/` 中保留的上游图片仅作为来源与设计参考，不能用于证明本 fork 的实际效果。上游所用 Nerd Fonts 等资源的名称、配色名称和相关标识也不构成对本 fork 的背书；具体资源的适用许可仍需在再分发前按其实际来源核实。

## Rust dependencies / Rust 依赖

Direct and transitive Rust dependencies are identified by `Cargo.toml` and the exact versions in `Cargo.lock`. They retain their respective licenses and notices. This file is not an exhaustive dependency-license audit and does not relabel every dependency as MIT. Verify the applicable license files and notice obligations of the versions actually distributed before publishing binaries.

Rust 依赖以 `Cargo.lock` 锁定的实际版本为准，各自保留其许可证与声明。本文件不声称已经完成全部传递依赖的许可审计。
