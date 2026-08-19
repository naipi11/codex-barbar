# codex-barbar

[English](README.md) · [简体中文](README.zh-CN.md)

**在 Windows 任务栏里，一眼看到你的 Codex 用量。**

codex-barbar 是一个轻量的 Windows 托盘应用，无需打开任何界面就能看到 Codex 每周用量与重置倒计时。它通过官方 Codex App Server 协议读取数据，凭据只保存在本机，安静地待在任务栏。

![codex-barbar 总览](docs/assets/hero-overview.svg)

*示意图 · 画面中的所有数值均为虚构演示数据。*

## 功能亮点

### 任务栏状态胶囊
一个紧凑、常驻 Windows 任务栏的胶囊，随时显示每周用量与重置倒计时。可随时在设置中关闭。

![任务栏状态胶囊](docs/assets/taskbar-capsule.svg)

### 动态悬浮球
置顶显示的轨道悬浮球，持续旋转。颜色表示剩余额度（绿色 · 充足，黄色 · 中等，红色 · 不足），转速反映活动状态（空闲、思考 ×2、极速 ×3）。支持开关，并可在设置中调整透明度与荧光亮度。

![悬浮球动画](docs/assets/float-orbit.gif)

![额度状态](docs/assets/float-orbit.svg)

### macOS 风格设置
圆角玻璃面板、克制的留白、清晰的开关，在 Windows 11 上呈现简洁的 macOS 质感。

![设置界面](docs/assets/settings-showcase.svg)

### 本地优先
凭据只保存在本机，并使用 Windows DPAPI（当前用户）保护。无遥测，更新只在你自己触发时检查。

![本地优先](docs/assets/privacy-local.svg)

## 快速开始

**系统要求**

- Windows 11 23H2 或更高版本，x64
- WebView2 Runtime（当前 Windows 11 已预装）
- 已安装 [Codex](https://developers.openai.com/codex/) 并通过 Codex CLI 登录（`codex login`）

**安装**

从 GitHub Releases 下载最新版本：

- 安装包：`codex-barbar_<版本>_x64-setup.exe`（当前用户 NSIS 安装，无需管理员权限）
- 便携版：`codex-barbar_<版本>_x64-portable.zip`

在提供 Authenticode 证书之前，二进制文件未签名，SmartScreen 可能显示警告。运行前请用同一发布中的 `SHA256SUMS.txt` 校验下载文件。

**首次使用**

1. 启动 codex-barbar，它驻留在系统托盘。
2. 单击托盘图标打开用量面板。
3. 如果面板显示“未登录”，请在终端中运行 `codex login` 后刷新。
4. 新安装默认开启任务栏状态条与悬浮球；两者都可在设置中关闭。

所有数据（包括便携版）都保存在 `%LOCALAPPDATA%\codex-barbar`。要删除全部账户与缓存，请先退出应用，再删除该目录；也可以在卸载时选择“是否删除本地 codex-barbar 账户与缓存？”的明确确认。

**设置**

- **通用** — 开机启动、任务栏状态、悬浮球、透明度、刷新间隔、显示模式、主题、语言
- **账户** — 管理已登录的 Codex 账户
- **高级** — Codex 可执行文件路径校验与诊断导出
- **关于** — 版本与更新检查

用量按可配置间隔自动刷新（默认 5 分钟），可能与线上账户存在延迟；需要最新数值时可在面板中手动刷新。

## V1 范围

- 仅支持 Codex。不包含其他服务商、浏览器 Cookie 导入、API Key 账户或用量通知界面。
- 通过官方但实验性的 `codex app-server` stdio JSONL 协议读取用量；`experimentalApi` 保持关闭，私有 `/wham/*` 调用已移除。
- 当前 CLI 档案只读。codex-barbar 不会替你登录、退出、切换或删除账户。
- 托管档案使用隔离的 `CODEX_HOME`，强制文件凭据存储，并使用 Windows DPAPI（仅当前用户）保护凭据。
- 无遥测。启动时不会检查、下载或应用更新；更新检查只能由你在界面中手动触发。

## 开发

构建前置条件与命令见 [docs/BUILDING.md](docs/BUILDING.md)，模块结构见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。技术栈：Tauri 2、React 18 / Vite、Rust，包管理器为 `pnpm@10.18.1`。

## 隐私与支持

- [PRIVACY.md](PRIVACY.md) — 数据存储位置与威胁边界
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) — 错误状态与恢复步骤
- [docs/TESTED_CODEX_VERSIONS.md](docs/TESTED_CODEX_VERSIONS.md) — 已测试的 Codex 版本范围

## 上游来源

codex-barbar 是 [CodexBar](https://github.com/steipete/CodexBar) 的 Windows 移植版。审计后的源码记录与冻结基线见 [UPSTREAMS.md](UPSTREAMS.md) 和 [docs/architecture/V1_BASELINE.md](docs/architecture/V1_BASELINE.md)。

## 许可证

MIT。见 [LICENSE](LICENSE)。