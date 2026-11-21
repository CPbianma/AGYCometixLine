# agyline 私有本地辅助入口

完整说明见仓库的 [中文主文档](../../README.zh.md)；[English quick start](../../README.md)。

源码位于 [CPbianma/AGYCometixLine](https://github.com/CPbianma/AGYCometixLine)。此目录不是已发布的 npm 产品，所有 `npm/` 包均为 `private: true`。`@cometix/ccline` 属于上游，不能用作 agyline 的安装源；本目录没有平台包依赖或远程二进制下载。

## 先构建并安装本 fork

在仓库根目录执行：

```sh
cargo build --release --locked
```

随后在 Windows 运行 `target/release/agyline.exe install`，在 macOS／Linux 运行 `./target/release/agyline install`。需要 Rust 1.89 或更新版本。完整的路径覆盖、备份与卸载说明见主文档。

## 可选：直接使用本地 helper

无需 npm 安装。在仓库根目录执行：

```sh
node npm/main/bin/ccline.js --help
node npm/main/bin/ccline.js preview --scenario antigravity --width 100 --color none
```

`ccline.js` 仅保留历史文件名；本地包的可选 bin 名为 `agyline-local`，不是上游 `ccline`，也不遮蔽真实 `agyline`。

helper 只运行 `AGYLINE_HOME` 目录中的 `agyline.exe`（Windows）或 `agyline`（Unix）；未设置时使用 `~/.gemini/antigravity-cli/agyline/`。它传递参数、继承终端并保留正常子进程退出码。程序缺失、启动失败或被信号终止均返回失败，不下载、不复制、不扫描 `.claude`、不寻找上游包、不回退 PATH。`AGY_CONFIG_DIR` 不会改变 helper 的可执行文件位置。

旧 `postinstall.js` 仅输出提示，不做安装或设置修改。`npm/scripts/prepare-packages.js` 始终返回失败，不生成发布目录；平台目录仅为私有占位元数据，不包含可用的预编译包。不要把这些包名写成公开 npm 安装命令。
