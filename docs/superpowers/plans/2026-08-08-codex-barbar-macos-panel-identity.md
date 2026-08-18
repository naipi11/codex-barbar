# codex-barbar macOS 风格面板与账号身份修复实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 Codex 账号显示、补充隐藏面板按钮，并把无边框 TrayPanel 重构为已确认的 macOS 菜单栏小卡片风格。

**Architecture:** 保持 Tauri `main` WebView、现有 `dismissTrayPanel`、账号服务和事件名称不变。Rust 在 `account/read` 成功后立即缓存并发布账号 snapshot；额度刷新独立发布用量/错误状态。React 通过扩展后的 `ProfileSummaryDto` 渲染身份状态，TrayPanel 只调整结构、交互和 CSS，不引入新的 UI 依赖。

**Tech Stack:** Rust 2024、Tauri 2、React 18、TypeScript、Vitest 3、Testing Library、Windows WebView2、现有 `secure_file`/DPAPI 身份缓存。

## Global Constraints

- 关闭按钮只隐藏面板，不退出后台进程；退出应用仍由独立的 `quitApp` 操作完成。
- 身份展示顺序固定为 `displayName -> email -> 已登录（名称不可用） -> 未登录`。
- 额度 403、超时或协议错误不得把已确认身份改写成“未登录”；旧额度必须保留并标记刷新失败。
- 不读取、记录或桥接 token、cookie、auth.json 原文、凭据路径或完整 RPC 响应。
- 不新增 Rust、npm、图标包或 UI 框架依赖。
- 不改变任务栏状态 overlay、动画悬浮球、设置键和现有事件名称的产品语义。
- Rust 变更后运行 `cargo fmt --all`、两个 crate 的测试和 Clippy；前端变更后运行 `pnpm test` 和 `pnpm run build`。
- UI 变更必须基于 fresh Windows 构建验证；CUA 不可用时记录 Win32/截图替代证据。

---

## 文件与职责映射

### 身份和刷新链路

- Modify: `rust/src/accounts/identity.rs` — 扩展缓存记录的登录状态/时间兼容读取。
- Modify: `rust/src/accounts/service.rs` — `account/read` 成功后发布账号 snapshot；额度失败时发布保留旧快照的用量错误状态。
- Modify: `rust/src/providers/codex/app_server/model.rs` — 补齐真实 `account/read` 形状和身份回退测试。
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` — 输出 `accountStatus`、`accountUpdatedAt` 和安全回退 label。
- Modify: `apps/desktop-tauri/src/types/bridge.ts` — 对齐桥接 DTO 类型和身份状态枚举。
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts` — 给所有 fixture 提供兼容身份字段。

### TrayPanel 交互和视觉

- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.tsx` — 增加 `TrayHeader` 结构和隐藏按钮。
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayHeader.tsx` — 工具栏、身份摘要、X 按钮的可测试单元。
- Modify: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx` — 将未样式化 select 替换为可访问的圆角 listbox/popover。
- Modify: `apps/desktop-tauri/src/surfaces/tray/QuotaCard.tsx` — 增加 macOS 卡片层级和状态 class。
- Modify: `apps/desktop-tauri/src/surfaces/tray/UsageStatus.tsx` — 将错误/刷新状态压缩为轻量状态行。
- Modify: `apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx` — 胶囊按钮和低强调退出操作。
- Modify: `apps/desktop-tauri/src/surfaces/tray/TrayPanel.css` — surface tokens、圆角、阴影、滚动和 reduced-motion。
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts` — 使用 `accountStatus` 区分已登录、未登录和不可用。

### 测试与验证

- Modify: `rust/src/accounts/identity.rs` tests — cache compatibility and status.
- Modify: `rust/src/accounts/service.rs` tests — identity event before quota failure.
- Modify: `rust/src/providers/codex/app_server/model.rs` tests — real response fixture shape.
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs` tests — DTO status/fallback/security.
- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx` — close/identity/status behavior.
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayHeader.test.tsx` — header and close semantics.
- Modify: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.test.tsx` — listbox keyboard behavior.
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx` — identity fallback matrix.

---

### Task 1: Extend the identity model and prove the real account response

**Files:**

- Modify: `rust/src/accounts/identity.rs`
- Modify: `rust/src/providers/codex/app_server/model.rs`
- Test: the `#[cfg(test)]` modules in both files

**Interfaces:**

- Produces `AccountIdentityRecord::status` with serialized values `signedIn`, `signedOut`, or `unavailable`, while old cache JSON without the field remains readable.
- Produces `AccountIdentity::from_value` coverage for `{ "account": { "type": "...", "email": "...", "planType": "..." } }`.

- [ ] **Step 1: Write the failing cache compatibility test**

Add a test that deserializes an old record containing only `display_name`, `email`, `plan_type`, and `updated_at`, then asserts `status == AccountStatus::Unavailable`.

- [ ] **Step 2: Run the focused Rust test and verify the expected failure**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::identity::tests::old_cache_record_defaults_to_unavailable -- --exact
```

Expected: compilation/test failure because `AccountIdentityRecord` has no `status` field or default.

- [ ] **Step 3: Write the failing App Server identity-status test**

Add a fixture value with only `account.type`, `account.email`, and `account.planType`. Assert that parsing returns `AuthMode::ChatGpt`, `display_name == None`, the email is present, the plan is present, and the new `AccountIdentity::status()` helper returns `AccountStatus::SignedIn`.

- [ ] **Step 4: Run the identity-status test and verify it fails for the missing contract**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml providers::codex::app_server::model::tests::chatgpt_account_with_email_only_is_signed_in -- --exact
```

Expected: the new test fails because `AccountIdentity::status()` does not exist yet.

- [ ] **Step 5: Implement the minimal model changes**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountStatus {
    SignedIn,
    SignedOut,
    Unavailable,
}
```

Add `status: AccountStatus` to `AccountIdentityRecord`, annotate it with `#[serde(default = "default_account_status")]`, and use `Unavailable` for old records. Set parser-level identity results to `SignedIn` when a supported account type is returned; retain `NotSignedIn` for explicit signed-out responses.

- [ ] **Step 6: Run the focused tests and commit**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::identity::tests providers::codex::app_server::model::tests
```

Expected: all focused tests pass. Commit:

```powershell
git add rust/src/accounts/identity.rs rust/src/providers/codex/app_server/model.rs
git commit -m "Model Codex account identity status"
```

---

### Task 2: Publish identity before quota failure and expose safe bridge fields

**Files:**

- Modify: `rust/src/accounts/service.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/commands/bridge.rs`
- Modify: `apps/desktop-tauri/src/types/bridge.ts`
- Modify: `apps/desktop-tauri/src/test/profileUsageFixtures.ts`
- Test: Rust account-service/bridge tests and frontend type fixtures

**Interfaces:**

- `ProfileSummaryDto.accountStatus: "signedIn" | "signedOut" | "unavailable"`.
- `ProfileSummaryDto.accountUpdatedAt: string | null`.
- `ProfileSummaryDto.label` never returns `Current CLI` for a Current CLI profile.
- `AccountProfileService::refresh_current_cli` publishes `ProfilesChanged` immediately after successful identity cache write.

- [ ] **Step 1: Write the failing service event test**

Extend the fake App Server test support with a response that succeeds for `account/read` and fails for `rateLimits/read`. Subscribe to the service event stream, call `request_refresh(current_cli_id, RefreshTrigger::Manual)`, and assert that a `ProfilesChanged` event is received before the usage error event.

- [ ] **Step 2: Run the service test and verify it fails**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::service::tests::identity_event_precedes_rate_limit_failure -- --exact
```

Expected: timeout/no `ProfilesChanged` event because the current code writes the cache but returns on the quota error without publishing the account snapshot.

- [ ] **Step 3: Write the failing bridge tests**

Add tests asserting:

```rust
assert_eq!(dto.account_status, "signedIn");
assert_eq!(dto.label, "user@example.com"); // fixture value only
assert_eq!(dto.account_updated_at.is_some(), true);
```

Also add a test where status is `SignedIn` but both name and email are absent; assert the label is `已登录（名称不可用）`, never `Current CLI`.

- [ ] **Step 4: Run the bridge tests and verify the new fields fail**

Run:

```powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge::tests
```

Expected: compile failure because the DTO and bridge types do not yet expose the new fields.

- [ ] **Step 5: Implement the minimal service and bridge changes**

In the account service:

1. Add a helper that sends `AccountServiceEvent::ProfilesChanged(self.snapshot()?)` after `cache_identity` succeeds.
2. Call it in both Current CLI and managed refresh paths before `rate_limits_read`.
3. Change refresh-error handling to load the profile usage state after `save_error` and emit `UsageStateChanged` so the WebView keeps the old snapshot while seeing the error.
4. Do not include identity values in logs.

In the bridge:

1. Serialize `account_status` and `account_updated_at` as camelCase.
2. Map the Rust status enum to the frozen string union.
3. Generate Current CLI labels with `displayName`, then `email`, then `已登录（名称不可用）` for signed-in identity, otherwise `未登录`.
4. Keep managed custom labels unchanged.

In TypeScript fixtures, default new fields to `accountStatus: "unavailable"` and `accountUpdatedAt: null`.

- [ ] **Step 6: Run focused tests and commit**

Run:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts::service::tests::identity_event_precedes_rate_limit_failure -- --exact
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml commands::bridge::tests
```

Expected: all focused tests pass. Commit:

```powershell
git add rust/src/accounts/service.rs apps/desktop-tauri/src-tauri/src/commands/bridge.rs apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/test/profileUsageFixtures.ts
git commit -m "Publish Codex identity independently from quota refresh"
```

---

### Task 3: Add close button and identity-aware status behavior

**Files:**

- Create: `apps/desktop-tauri/src/surfaces/tray/TrayHeader.tsx`
- Create: `apps/desktop-tauri/src/surfaces/tray/TrayHeader.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.ts`
- Modify: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx`
- Modify: `apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx`

**Interfaces:**

`TrayHeaderProps`:

```ts
interface TrayHeaderProps {
  productName: string;
  version: string;
  profile: ProfileSummaryDto | null;
  onDismiss(): Promise<void> | void;
}
```

`profileDisplayName(profile)` returns the display string; `profileStatusLabel(profile)` returns the localized status string.

- [ ] **Step 1: Write the failing header test**

Render `TrayHeader` with a signed-in email-only profile and assert:

```ts
expect(screen.getByText("user@example.com")).toBeInTheDocument();
expect(screen.getByRole("button", { name: /隐藏面板|close/i })).toBeInTheDocument();
```

Click the button and assert the injected `onDismiss` spy is called once.

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- src/surfaces/tray/TrayHeader.test.tsx
```

Expected: module/element failure because `TrayHeader` does not exist.

- [ ] **Step 3: Write the failing identity fallback tests**

Add cases for:

1. display name wins over email;
2. email wins when display name is null;
3. signed-in profile without both values returns `已登录（名称不可用）`;
4. signed-out profile returns `未登录`;
5. unavailable profile returns `账号信息不可用`.

- [ ] **Step 4: Run the fallback tests and verify the missing status behavior**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useStatusSurface.test.tsx
```

Expected: the new signed-in-without-name case fails because the current helper only returns `未登录`.

- [ ] **Step 5: Implement the header and hook behavior**

Create `TrayHeader` with a `<button type="button">` that calls `onDismiss`, `aria-label="隐藏面板"`, and no `quitApp` reference. Move account summary markup out of the main header into this component.

Update `profileDisplayName` and add `profileStatusLabel` to use `accountStatus`. Update `TrayPanel` to find the selected profile through `useProfileUsage` and pass it to `TrayHeader`.

Keep Escape listener calling `dismissTrayPanel`; do not add a second close command.

- [ ] **Step 6: Run focused tests and commit**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- src/surfaces/tray/TrayHeader.test.tsx src/hooks/useStatusSurface.test.tsx src/surfaces/TrayPanel.test.tsx
```

Expected: all focused tests pass. Commit:

```powershell
git add apps/desktop-tauri/src/surfaces/tray/TrayHeader.tsx apps/desktop-tauri/src/surfaces/tray/TrayHeader.test.tsx apps/desktop-tauri/src/surfaces/TrayPanel.tsx apps/desktop-tauri/src/hooks/useStatusSurface.ts apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx
git commit -m "Add tray panel close action and identity states"
```

---

### Task 4: Rebuild the TrayPanel visual system

**Files:**

- Modify: `apps/desktop-tauri/src/surfaces/tray/TrayPanel.css`
- Modify: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/tray/ProfileSelector.test.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/tray/QuotaCard.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/tray/UsageStatus.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx`
- Modify: `apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx`

**Interfaces:**

- Existing bridge commands remain unchanged: `dismissTrayPanel`, `refreshSelectedProfile`, `openCodexUsagePage`, `openSettingsWindow`, `quitApp`.
- Existing roles remain available: `region`, `progressbar`, `combobox`/`listbox`, and `button`.
- CSS state classes are limited to `ready`, `warning`, `critical`, `missing`, `refreshing`, and `stale`.

- [ ] **Step 1: Write the failing visual/interaction assertions**

Add tests asserting:

```ts
expect(screen.getByRole("button", { name: /隐藏面板|close/i })).toHaveClass("tray-panel__close");
expect(screen.getByRole("main", { name: /codex-barbar tray panel/i })).toHaveClass("tray-panel--macos");
expect(screen.getByRole("region", { name: /account/i })).toHaveClass("tray-account--card");
expect(screen.getByRole("progressbar")).toHaveClass("quota-card--ready");
```

Add a profile selector test that opens the listbox, moves with ArrowDown, selects with Enter, and closes with Escape while restoring focus to the trigger.

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```powershell
pnpm --dir apps/desktop-tauri test -- src/surfaces/TrayPanel.test.tsx src/surfaces/tray/ProfileSelector.test.tsx
```

Expected: class/listbox assertions fail because the new macOS classes and custom listbox do not exist yet.

- [ ] **Step 3: Implement the minimal accessible markup**

Use a button trigger with `aria-haspopup="listbox"` and `aria-expanded`. Render the options in a positioned listbox with `role="option"` and `aria-selected`; maintain a ref to the trigger so Escape returns focus.

Preserve the existing `onSelect(profileId)` contract and do not change profile switching semantics.

- [ ] **Step 4: Replace the CSS with the approved surface tokens**

Add the tokens from the spec and implement:

- 16–20px radius and soft shadow on `.tray-panel`;
- fixed toolbar with visible close button;
- muted surfaces for account/quota/status cards;
- gradient progress fill with status color;
- capsule buttons;
- internal scroll area and `prefers-reduced-motion` override;
- focus-visible outline using `--tray-accent`.

Keep dark/light/system theme variables supplied by `useTheme`; do not hard-code a second theme switch.

- [ ] **Step 5: Run the frontend suite and build**

Run:

```powershell
pnpm --dir apps/desktop-tauri test
pnpm --dir apps/desktop-tauri run build
```

Expected: all frontend tests pass and the production build completes.

- [ ] **Step 6: Commit the visual refactor**

```powershell
git add apps/desktop-tauri/src/surfaces/tray/TrayPanel.css apps/desktop-tauri/src/surfaces/tray/ProfileSelector.tsx apps/desktop-tauri/src/surfaces/tray/ProfileSelector.test.tsx apps/desktop-tauri/src/surfaces/tray/QuotaCard.tsx apps/desktop-tauri/src/surfaces/tray/UsageStatus.tsx apps/desktop-tauri/src/surfaces/tray/TrayActions.tsx apps/desktop-tauri/src/surfaces/TrayPanel.test.tsx
git commit -m "Restyle tray panel with macOS card surfaces"
```

---

### Task 5: Full verification and Windows fresh-build proof

**Files:**

- Modify only if verification exposes a defect in the files above.
- Evidence: `docs/verification/codex-barbar-macos-panel-identity-2026-08-08.md`

**Interfaces:**

- No new public interface; verification consumes the completed Rust/Tauri/React behavior.

- [ ] **Step 1: Run format and Rust gates**

Run:

```powershell
cargo fmt --all --check
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: all commands exit 0.

- [ ] **Step 2: Run the local CI slice**

Run:

```powershell
.\scripts\local-check.ps1
```

Expected: Rust, Tauri and frontend checks pass; if a hosted-only step is unavailable, record the exact skipped command.

- [ ] **Step 3: Build a fresh Windows debug installer**

Run:

```powershell
pnpm --dir apps/desktop-tauri run tauri:build:debug
```

Close any running codex-barbar process before launch so the single-instance plugin does not hand automation to a stale binary.

- [ ] **Step 4: Capture UI proof**

Launch the newly built binary with:

```powershell
$env:CODEXBAR_PROOF_MODE = 'settings:menu'
```

Verify the tray panel screenshot, close button, internal scroll, identity fallback, and quota-error separation. Then open the panel again from the tray/overlay and verify the process remains alive after X.

If CUA is available, use the driver window list, UIA tree and screenshot commands. If not, use `Get-Process`, `Get-CimInstance Win32_Process`, Win32 window enumeration, and `PrintWindow` capture.

- [ ] **Step 5: Record evidence and commit**

Write exact commands, binary path, observed UI behavior, and any CUA limitation to:

```text
docs/verification/codex-barbar-macos-panel-identity-2026-08-08.md
```

Run:

```powershell
git add docs/verification/codex-barbar-macos-panel-identity-2026-08-08.md
git commit -m "Verify macOS panel and identity fix on Windows"
```

---

## Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-08-codex-barbar-macos-panel-identity.md`.

Execution choices:

1. **Subagent-Driven (recommended):** dispatch a fresh agent per task and review after each task.
2. **Inline Execution:** execute tasks in this session using `superpowers:executing-plans` with checkpoints.
