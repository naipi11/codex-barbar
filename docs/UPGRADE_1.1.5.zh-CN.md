# codex-barbar 1.1.5 升级说明

## 版本与范围

- 维护版本：**1.1.5**；源码更新不等于正式发布，本次未创建版本标签或 GitHub Release。
- 兼容目标：**Codex CLI 0.167.0**，这是 2026-11-08 核实的[官方最新稳定版](https://github.com/openai/codex/releases/tag/rust-v0.167.0)。
- 从仓库 main 的 1.1.4 开始修改，保留托盘、悬浮球、任务栏、账户、主题和语言功能。
- 不需要删除账户数据库或迁移全局 Codex 配置；本轮未替用户执行登录、登出或切换账户。

## 主要改进

1. **协议兼容**：修正握手 capabilities 层级，保持 experimentalApi=false；支持官方字符串/整数请求 ID，对不支持的服务端请求明确拒绝，不授权工具操作。
2. **登录结果准确**：检查登录完成通知的 success，不把失败当作成功；保留脱敏错误，进程退出时结束等待。
3. **额度更稳健**：合法 null 窗口不再误报；金额、消费 credits 不再被当作额度百分比；坏的附加额度桶不影响主额度；保留桶身份和 reset credits 可用次数。
4. **错误可见**：API-key 不再请求不支持的订阅额度；空额度/解析失败会发布错误并保留原缓存，不会静默维持刷新成功状态。
5. **自定义路径生效**：每次会话读取最新设置；npm shim 保存为所选绝对路径，不再误保存 node.exe；验证失败保留旧设置。
6. **响应与资源优化**：版本探测限 5 秒/4 KiB并回收进程树；设置/诊断探测移至工作线程；取消、超时、断连清理请求和后台任务；超长 stderr 有界并持续排空，避免堵塞。
7. **发布优化生效**：原有 LTO、单 codegen unit、strip 配置移至 Cargo workspace 根，修复成员配置被忽略的问题。未新增项目依赖。

## 验证证据

环境：Windows 11 x64，OS 10.0.26200.0，Rust 1.97.1，Node 24.21.0，项目 pnpm 10.18.1。Node 与 CI 的 Node 20 不同，发布前仍建议运行仓库 CI。

- 真实 Codex 0.167.0：握手、当前已登录账户读取、额度读取成功。
- 隔离空 CODEX_HOME：握手成功，正确报告未登录；这不是已登录额度验证。
- 前端 39 个文件/299 项测试和生产 Web 构建通过。
- 生产依赖漏洞检查、许可证审计、V1/release-workflow 边界检查通过。
- Rust 最终测试、Clippy 和桌面验收见 [Windows 验收记录](WINDOWS_ACCEPTANCE.md)；版本证据见 [兼容矩阵](TESTED_CODEX_VERSIONS.md)。

**没有证明的范围**：真实托管账户的浏览器/设备码完整登录、各种企业代理与 keyring 组合、GCP/Bedrock 订阅额度、完整多屏/DPI/安装升级卸载矩阵。离线协议回归测试不等同于这些真实环境实测。

## 本地构建产物

安装包、便携版和校验值已在本地交付，二进制产物不提交到源码仓库，也尚未上传 GitHub Release。可按下方复现命令重新构建。

```text
artifacts/1.1.5/codex-barbar_1.1.5_x64-setup.exe
artifacts/1.1.5/codex-barbar_1.1.5_x64-portable.zip
artifacts/1.1.5/SHA256SUMS.txt
```

安装包未进行 Authenticode 签名，Windows 可能显示 SmartScreen 提示。SHA-256 用于校验文件完整性，不代表微软签名认证。用户已反馈安装成功；这不替代完整安装、升级、卸载矩阵验证。

## 使用与升级

1. 运行 `codex --version`。本机已经是 0.167.0，本轮没有替换 Codex 安装。
2. 退出旧版 codex-barbar，再启动新构建或运行按用户安装包。单实例机制可能把第二次启动交给旧进程，不要同时保留旧版运行。
3. 账号数据沿用 `%LOCALAPPDATA%/codex-barbar`，不要删除该目录。安装前建议自行备份；本轮未覆盖安装目录。
4. 如有需要，在**设置 → 高级**校验 Codex 绝对路径，然后刷新。设置在下次会话生效，不需要重启。
5. 若 Codex 自身因旧配置无法启动，按[官方配置文档](https://learn.chatgpt.com/docs/config-file/config-basic)排查，不要为额度显示关闭安全审批。

## 复现检查

```powershell
cargo fmt --all --check
cargo test --locked --manifest-path rust/Cargo.toml
cargo test --locked --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --locked --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo clippy --locked --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm --dir apps/desktop-tauri install --frozen-lockfile
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run tauri:build
cargo build --locked --manifest-path rust/Cargo.toml --example codex_app_server_smoke
./target/debug/examples/codex_app_server_smoke.exe
```

最后一条只通过官方 App Server 读取当前 CLI，输出脱敏状态，不显示邮箱、账号 ID、密钥或具体额度。隔离未登录验证可设置一个新建的绝对 CODEX_HOME 并加 `--allow-signed-out`，不能将其标记为已登录额度实测。

桌面截图使用合成数据 proof 模式。绝对 `CODEXBAR_PROOF_DATA_DIR` 将数据库等隔离到其 `codex-barbar` 子目录，并禁止 proof 更改自启注册表；日志仍在默认日志目录。
