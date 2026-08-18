# codex-barbar macOS 风格面板与账户身份验证

验证日期：2026-08-09（Asia/Shanghai）  
分支：`codex/taskbar-floatball-identity`

## 自动化验证

以下命令均在 fresh worktree `C:\Users\stack\Documents\codex-barbar\.worktrees\taskbar-floatball-identity` 执行：

- `cargo fmt --all --check`：通过。
- `cargo test --manifest-path rust/Cargo.toml`：267 passed、1 ignored、0 failed；另有 17 个 app-server contract tests 通过。
- `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`：通过。
- `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`：96 passed、0 failed。
- `cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings`：通过。
- `pnpm --dir apps/desktop-tauri exec vitest run`：21 个测试文件、77 tests passed、0 failed。
- `pnpm --dir apps/desktop-tauri run build`：TypeScript 检查和 Vite production build 通过。
- `./scripts/local-check.ps1`：V1 boundary guard、Rust/Tauri/前端测试和前端构建全部通过，最终输出 `Local checks passed.`。

## Fresh Windows 构建

命令：

```powershell
$env:CI = 'true'
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

结果：成功生成：

```text
C:\Users\stack\Documents\codex-barbar\.worktrees\taskbar-floatball-identity\target\debug\codex-barbar.exe
C:\Users\stack\Documents\codex-barbar\.worktrees\taskbar-floatball-identity\target\debug\bundle\nsis\codex-barbar_1.0.0_x64-setup.exe
```

NSIS 静默运行返回 `ExitCode=0`。由于安装器和已安装程序都是 `1.0.0`，同版本覆盖没有改变已安装文件哈希；随后保留旧文件备份并写入 fresh EXE，最终 fresh 与安装目录哈希一致：

```text
Installed: C:\Users\stack\AppData\Local\Programs\codex-barbar\codex-barbar.exe
Backup:    C:\Users\stack\AppData\Local\Temp\codex-barbar-verification\codex-barbar-installed-before-fresh.exe
SHA-256:   841B96914B980C0149445B1E7982A3B1D63C544B3A4910A6BEF8724F83C89FA2
```

## Windows 交互证据

本机未发现 CUA driver：

```text
C:\Users\stack\AppData\Local\Programs\Cua\cua-driver\bin\cua-driver.exe -> CUA_DRIVER_NOT_FOUND
```

因此使用 Windows 原生窗口状态作为关闭行为的替代验证，并使用 fresh proof 启动：

```powershell
$env:CODEXBAR_PROOF_MODE = 'trayPanel:ready'
Start-Process target\debug\codex-barbar.exe
```

原生窗口消息验证结果：

- `BeforeVisible=True`
- 发送 Escape（与 X 按钮共用 `dismissTrayPanel` 路径）后：`AfterVisible=False`
- 面板隐藏后：`AliveAfterDismiss=True`
- 再次显示窗口后：`BeforeHidden=True`、`ReopenedVisible=True`、`AliveAfterReopen=True`

这证明关闭动作只隐藏面板，托盘进程仍然驻留，并可再次打开。

视觉截图（由用户在 fresh proof 运行中手动截取，账户内容为无凭据 synthetic fixture）保存在：

[codex-barbar-macos-panel-proof-user.png](./windows/codex-barbar-macos-panel-proof-user.png)

截图确认：

- 顶部显示账户头像、账户名和邮箱，而不是 `Current CLI` / `未登录`。
- 右上角有可见的圆形 X。
- 账户、5 小时额度、每周额度和数据状态使用圆角卡片布局。
- 进度条使用渐变填充，面板内部可滚动。

## 限制

CUA 驱动未安装，因此没有 UIA 树或 CUA click trace；关闭按钮本身由 Vitest 直接点击测试覆盖，Windows 原生替代验证覆盖了同一隐藏/重开窗口路径和进程存活语义。
