# Glass Orbit Status Surfaces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将任务栏状态胶囊和悬浮球升级为 Glass Orbit 双额度界面，并保证设置开关、表面 X、原生关闭请求和后台监控始终收敛到同一持久化状态。

**Architecture:** 后端新增可测试的 StatusSurfaceController，将窗口生命周期、SQLite 设置更新和 settings-changed 事件编排为带回滚的事务；TaskbarOverlay 与 FloatBall 继续分别拥有定位逻辑。前端新增纯 StatusSurfaceViewModel 和 typed invoke bridge，任务栏与悬浮球只消费同一份双额度模型；悬浮球展开由 React 延迟状态机驱动、由 Rust 原子更新窗口矩形。

**Tech Stack:** Rust 2024、Tauri 2.10、React 18、TypeScript 5.6、Vitest 3、Testing Library、Windows Win32/DWM、SQLite SettingsRepository、pnpm 10.18.1。

## Global Constraints

- 工作目录固定为 C:\Users\stack\Documents\codex-barbar\.worktrees\taskbar-floatball-identity，分支固定为 codex/taskbar-floatball-identity。
- 设计基线是 docs/superpowers/specs/2026-08-09-status-surfaces-glass-orbit-design.md；实现不得改变已批准的关闭语义。
- 不引入任何新的 Rust、npm、UI、图标或动画依赖；前端继续使用 pnpm@10.18.1。
- 不使用 Explorer DLL 注入；任务栏状态仍是通知区域左侧的非激活 overlay。
- 设置字段名保持 taskbarStatusEnabled 与 floatBallEnabled；Rust 字段保持 taskbar_status_enabled 与 float_ball_enabled。
- 任务栏目标逻辑尺寸约 318 × 44；悬浮球收起 88 × 88、展开 260 × 148。
- 悬浮球悬停展开延迟固定为 180ms；后台状态收敛周期保持 2 秒。
- float-ball WebView builder 必须保留 theme(Some(tauri::Theme::Dark))，防止共享 WebView2 profile 改写其他窗口的 prefers-color-scheme。
- 额度紧迫度始终按 usedPercent；显示数字遵循 settings.displayMode。
- 账号身份只能来自当前选中的 profile；额度错误不得把已确认身份改成未登录。
- UI 改动完成后必须关闭旧实例、fresh debug build，并使用 CUA/Computer Use 做 Windows 实机证明。
- 每个任务遵循 RED → 最小实现 → GREEN → 独立 commit；不把多个任务压成一个提交。

---

## File Responsibility Map

### 新建文件

| 文件 | 单一职责 |
|---|---|
| apps/desktop-tauri/src-tauri/src/status_surfaces/window_lifecycle.rs | 独立执行 hide/destroy 并把结果归一化为可测试的关闭结果 |
| apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs | 编排表面启停、设置持久化、失败回滚与 settings-changed 事件 |
| apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs | 暴露 set_status_surface_enabled 与 set_float_ball_expanded Tauri commands |
| apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts | 从选中 profile、usage state、display mode 派生双额度展示模型 |
| apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts | 覆盖额度识别、紧迫度、状态、时间和缺失数据 |
| apps/desktop-tauri/src/hooks/useFloatBallExpansion.ts | 管理 180ms 展开、延迟收起、取消 timer 和后端窗口同步 |
| apps/desktop-tauri/src/hooks/useFloatBallExpansion.test.tsx | 用 fake timers 固定悬停状态机 |

### 主要修改文件

| 文件 | 责任变化 |
|---|---|
| apps/desktop-tauri/src-tauri/src/status_surfaces.rs:1-170 | 汇总 manager、controller、monitor 和窗口事件入口 |
| apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs:1-69 | 真正关闭/销毁窗口；disabled 时继续清理残留 |
| apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs:1-66 | 318px 窗口契约和 hide/destroy 原语 |
| apps/desktop-tauri/src-tauri/src/float_ball/mod.rs:1-252 | 关闭生命周期、展开状态、保存 collapsed 几何 |
| apps/desktop-tauri/src-tauri/src/float_ball/window.rs:1-86 | 动态 Rect 更新和程序化 destroy |
| apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs:1-319 | 88px 收起矩形、260×148 展开锚定和 DPI clamp |
| apps/desktop-tauri/src-tauri/src/commands/settings.rs:1-165 | 将旧 surface patch 转交统一 controller |
| apps/desktop-tauri/src-tauri/src/main.rs:115-282 | 注册新命令并把 CloseRequested 路由为永久关闭 |
| apps/desktop-tauri/src/types/bridge.ts:37-101 | 新增 StatusSurfaceKind，保持现有 DTO 不变 |
| apps/desktop-tauri/src/lib/tauri.ts:1-137 | 新增两个 typed invoke 函数 |
| apps/desktop-tauri/src/hooks/useSettings.ts:1-58 | 设置页表面开关调用专用命令 |
| apps/desktop-tauri/src/hooks/useStatusSurface.ts:1-167 | 输出 StatusSurfaceViewModel 和表面操作 |
| apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx:1-33 | 双额度胶囊、独立主按钮与 X |
| apps/desktop-tauri/src/surfaces/TaskbarStatus.css:1-95 | Glass Orbit 任务栏视觉及窄宽度降级 |
| apps/desktop-tauri/src/surfaces/FloatBall.tsx:1-120 | 收起/展开 DOM、拖动、点击和独立 X |
| apps/desktop-tauri/src/surfaces/FloatBall.css:1-121 | Glass Orbit 圆环、展开卡片和 reduced motion |
| apps/desktop-tauri/src-tauri/src/proof_harness.rs | 提供 ready/warning/critical/refreshing/stale/missing 的无凭据 proof 场景 |
| docs/WINDOWS_PROOF.md | 更新尺寸、命令和验收矩阵 |

---

### Task 1: Make Auxiliary Window Teardown Deterministic

**Files:**
- Create: apps/desktop-tauri/src-tauri/src/status_surfaces/window_lifecycle.rs
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs:1-15
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs:8-69
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs:4-66
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs:31-137
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/window.rs:5-86
- Test: apps/desktop-tauri/src-tauri/src/status_surfaces/window_lifecycle.rs
- Test: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs:207-252

**Interfaces:**
- Consumes: tauri::WebviewWindow::hide()、tauri::WebviewWindow::destroy()、现有窗口 label。
- Produces: CloseOutcome::{Destroyed, HiddenPendingDestroy}、hide_and_destroy(window)；TaskbarOverlay::is_enabled() 与 FloatBall::is_enabled()。

- [ ] **Step 1: Write failing lifecycle result tests**

~~~rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroy_success_wins_when_hide_fails() {
        let outcome = hide_and_destroy_with(|| Err(()), || Ok(())).unwrap();
        assert_eq!(outcome, CloseOutcome::Destroyed);
    }

    #[test]
    fn hidden_window_can_be_reconciled_when_destroy_fails() {
        let outcome = hide_and_destroy_with(|| Ok(()), || Err(())).unwrap();
        assert_eq!(outcome, CloseOutcome::HiddenPendingDestroy);
    }

    #[test]
    fn close_fails_only_when_hide_and_destroy_both_fail() {
        assert_eq!(
            hide_and_destroy_with(|| Err(()), || Err(())).unwrap_err(),
            "STATUS_SURFACE_WINDOW_CLOSE_FAILED"
        );
    }
}
~~~

- [ ] **Step 2: Run the focused Rust test and observe RED**

Run:

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml window_lifecycle
~~~

Expected: FAIL because status_surfaces::window_lifecycle and hide_and_destroy_with do not exist.

- [ ] **Step 3: Implement the close result normalizer**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    Destroyed,
    HiddenPendingDestroy,
}

pub fn hide_and_destroy_with(
    hide: impl FnOnce() -> Result<(), ()>,
    destroy: impl FnOnce() -> Result<(), ()>,
) -> Result<CloseOutcome, &'static str> {
    let hidden = hide().is_ok();
    let destroyed = destroy().is_ok();
    if destroyed {
        Ok(CloseOutcome::Destroyed)
    } else if hidden {
        Ok(CloseOutcome::HiddenPendingDestroy)
    } else {
        Err("STATUS_SURFACE_WINDOW_CLOSE_FAILED")
    }
}

pub fn hide_and_destroy(
    window: &tauri::WebviewWindow,
) -> Result<CloseOutcome, &'static str> {
    hide_and_destroy_with(
        || window.hide().map_err(|_| ()),
        || window.destroy().map_err(|_| ()),
    )
}
~~~

Declare it from status_surfaces.rs:

~~~rust
pub(crate) mod window_lifecycle;
~~~

- [ ] **Step 4: Make both managers use the same close primitive**

Add this cached-or-labeled helper beside hide_and_destroy:

~~~rust
pub fn close_cached_or_labeled(
    app: &tauri::AppHandle,
    cached: &mut Option<tauri::WebviewWindow>,
    label: &str,
) -> Result<CloseOutcome, String> {
    let Some(window) = cached
        .as_ref()
        .cloned()
        .or_else(|| app.get_webview_window(label))
    else {
        return Ok(CloseOutcome::Destroyed);
    };
    let outcome = hide_and_destroy(&window).map_err(str::to_string)?;
    *cached = None;
    Ok(outcome)
}
~~~

TaskbarOverlay::apply_enabled(false) sets self.enabled=false, calls close_cached_or_labeled with taskbar_overlay::window::TASKBAR_WINDOW_LABEL, restores the previous enabled value on Err, and clears last_slot on success.

FloatBall::apply_enabled(false) records the previous state.enabled, calls state.apply_enabled(false), invokes close_cached_or_labeled with float_ball::window::FLOAT_BALL_WINDOW_LABEL, restores state.apply_enabled(previous_enabled) on Err, and never clears logical_position.

Both get_or_create builders change closable(false) to closable(true), while decorations(false) remains unchanged.

- [ ] **Step 5: Preserve the collapsed position test**

~~~rust
#[test]
fn disabling_does_not_discard_collapsed_position() {
    let mut state = FloatBallState::default();
    state.remember_logical_position(Point { x: -240, y: 96 });
    assert_eq!(state.apply_enabled(false), VisibilityIntent::Hide);
    assert_eq!(state.logical_position(), Some(Point { x: -240, y: 96 }));
}
~~~

- [ ] **Step 6: Run focused tests and format**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml window_lifecycle
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml disabling_does_not_discard_collapsed_position
cargo fmt --all
~~~

Expected: PASS.

- [ ] **Step 7: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/status_surfaces/window_lifecycle.rs apps/desktop-tauri/src-tauri/src/taskbar_overlay apps/desktop-tauri/src-tauri/src/float_ball
git commit -m "Fix auxiliary window teardown"
~~~

---

### Task 2: Add the Transactional StatusSurfaceController

**Files:**
- Create: apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs
- Create: apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs:1-170
- Modify: apps/desktop-tauri/src-tauri/src/commands/mod.rs:1-20
- Modify: apps/desktop-tauri/src-tauri/src/main.rs:115-141
- Test: apps/desktop-tauri/src-tauri/src/status_surfaces/controller.rs
- Test: apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs

**Interfaces:**
- Consumes: StatusSurfaceState managers、SettingsRepository、events::SETTINGS_CHANGED。
- Produces: StatusSurfaceKind::{TaskbarStatus, FloatBall}；transition(runtime, store, surface, enabled)；set_status_surface_enabled(surface, enabled) -> AppSettingsDto。

- [ ] **Step 1: Write failing transaction tests with fakes**

~~~rust
use std::cell::{Cell, RefCell};

#[derive(Default)]
struct FakeRuntime {
    calls: Vec<(StatusSurfaceKind, bool)>,
    fail_on: Option<bool>,
}

impl FakeRuntime {
    fn enabled() -> Self {
        Self::default()
    }

    fn failing_on(mut self, enabled: bool) -> Self {
        self.fail_on = Some(enabled);
        self
    }

    fn calls(&self) -> &[(StatusSurfaceKind, bool)] {
        &self.calls
    }
}

impl SurfaceRuntime for FakeRuntime {
    fn apply(
        &mut self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<(), String> {
        self.calls.push((surface, enabled));
        if self.fail_on == Some(enabled) {
            Err("STATUS_SURFACE_WINDOW_CLOSE_FAILED".to_string())
        } else {
            Ok(())
        }
    }
}

struct FakeStore {
    saved: RefCell<AppSettings>,
    fail_save: bool,
    save_count: Cell<usize>,
}

impl FakeStore {
    fn with_settings(saved: AppSettings) -> Self {
        Self {
            saved: RefCell::new(saved),
            fail_save: false,
            save_count: Cell::new(0),
        }
    }

    fn failing_save(mut self) -> Self {
        self.fail_save = true;
        self
    }

    fn saved(&self) -> AppSettings {
        self.saved.borrow().clone()
    }

    fn save_count(&self) -> usize {
        self.save_count.get()
    }
}

impl SurfaceSettingsStore for FakeStore {
    fn load(&self) -> Result<AppSettings, String> {
        Ok(self.saved())
    }

    fn write_enabled(
        &self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<AppSettings, String> {
        if self.fail_save {
            return Err("STATUS_SURFACE_SETTINGS_SAVE_FAILED".to_string());
        }
        self.save_count.set(self.save_count.get() + 1);
        let mut next = self.saved();
        match surface {
            StatusSurfaceKind::TaskbarStatus => {
                next.taskbar_status_enabled = enabled;
            }
            StatusSurfaceKind::FloatBall => {
                next.float_ball_enabled = enabled;
            }
        }
        *self.saved.borrow_mut() = next.clone();
        Ok(next)
    }
}

fn settings(taskbar: bool, float_ball: bool) -> AppSettings {
    AppSettings {
        taskbar_status_enabled: taskbar,
        float_ball_enabled: float_ball,
        ..AppSettings::default()
    }
}

#[test]
fn runtime_failure_does_not_persist_false() {
    let mut runtime = FakeRuntime::enabled().failing_on(false);
    let store = FakeStore::with_settings(settings(true, false));

    let error = transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .unwrap_err();

    assert_eq!(error, "STATUS_SURFACE_WINDOW_CLOSE_FAILED");
    assert!(store.saved().taskbar_status_enabled);
}

#[test]
fn persistence_failure_restores_previous_runtime_state() {
    let mut runtime = FakeRuntime::enabled();
    let store = FakeStore::with_settings(settings(true, false)).failing_save();

    assert!(transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .is_err());
    assert_eq!(runtime.calls(), &[(StatusSurfaceKind::TaskbarStatus, false),
                                 (StatusSurfaceKind::TaskbarStatus, true)]);
}

#[test]
fn already_persisted_false_still_reconciles_runtime_without_rewriting() {
    let mut runtime = FakeRuntime::enabled();
    let store = FakeStore::with_settings(settings(false, false));

    transition(
        &mut runtime,
        &store,
        StatusSurfaceKind::TaskbarStatus,
        false,
    )
    .unwrap();

    assert_eq!(store.save_count(), 0);
    assert_eq!(runtime.calls(), &[(StatusSurfaceKind::TaskbarStatus, false)]);
}
~~~

- [ ] **Step 2: Run RED**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller
~~~

Expected: FAIL because controller.rs and its traits are absent.

- [ ] **Step 3: Define the frozen enum and transaction ports**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusSurfaceKind {
    TaskbarStatus,
    FloatBall,
}

pub trait SurfaceRuntime {
    fn apply(
        &mut self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<(), String>;
}

pub trait SurfaceSettingsStore {
    fn load(&self) -> Result<codexbar::storage::AppSettings, String>;
    fn write_enabled(
        &self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<codexbar::storage::AppSettings, String>;
}
~~~

Define the field mapping once:

~~~rust
impl StatusSurfaceKind {
    fn enabled_in(self, settings: &AppSettings) -> bool {
        match self {
            Self::TaskbarStatus => settings.taskbar_status_enabled,
            Self::FloatBall => settings.float_ball_enabled,
        }
    }

    fn patch(self, enabled: bool) -> SettingsPatch {
        match self {
            Self::TaskbarStatus => SettingsPatch {
                taskbar_status_enabled: Some(enabled),
                ..SettingsPatch::default()
            },
            Self::FloatBall => SettingsPatch {
                float_ball_enabled: Some(enabled),
                ..SettingsPatch::default()
            },
        }
    }
}

impl SurfaceSettingsStore for SettingsRepository {
    fn load(&self) -> Result<AppSettings, String> {
        SettingsRepository::load(self)
            .map_err(|_| "STATUS_SURFACE_SETTINGS_LOAD_FAILED".to_string())
    }

    fn write_enabled(
        &self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<AppSettings, String> {
        self.update(surface.patch(enabled))
            .map_err(|_| "STATUS_SURFACE_SETTINGS_SAVE_FAILED".to_string())
    }
}
~~~

This implementation builds exactly one SettingsPatch field and never returns raw paths or SQLite text.

- [ ] **Step 4: Implement the rollback transaction**

~~~rust
pub fn transition<R, S>(
    runtime: &mut R,
    store: &S,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettings, String>
where
    R: SurfaceRuntime,
    S: SurfaceSettingsStore,
{
    let previous = store.load()?;
    let previous_enabled = surface.enabled_in(&previous);
    runtime.apply(surface, enabled)?;

    if previous_enabled == enabled {
        return Ok(previous);
    }

    match store.write_enabled(surface, enabled) {
        Ok(settings) => Ok(settings),
        Err(error) => {
            if runtime.apply(surface, previous_enabled).is_err() {
                return Err("STATUS_SURFACE_ROLLBACK_FAILED".to_string());
            }
            Err(error)
        }
    }
}
~~~

Implement StatusSurfaceKind::enabled_in and StatusSurfaceKind::patch so later code never duplicates field matching.

- [ ] **Step 5: Add the production adapter and event emission**

~~~rust
struct TauriSurfaceRuntime<'a> {
    app: &'a tauri::AppHandle,
    state: &'a mut StatusSurfaceState,
}

impl SurfaceRuntime for TauriSurfaceRuntime<'_> {
    fn apply(
        &mut self,
        surface: StatusSurfaceKind,
        enabled: bool,
    ) -> Result<(), String> {
        match surface {
            StatusSurfaceKind::TaskbarStatus => {
                self.state.taskbar.apply_enabled(self.app, enabled)
            }
            StatusSurfaceKind::FloatBall => {
                self.state.float_ball.apply_enabled(self.app, enabled)
            }
        }
    }
}

fn settings_repository(
    app: &tauri::AppHandle,
) -> Result<SettingsRepository, String> {
    app.state::<Mutex<crate::state::AppState>>()
        .lock()
        .map_err(|_| "STATUS_SURFACE_SETTINGS_UNAVAILABLE".to_string())?
        .account_service
        .as_ref()
        .map(|service| service.repositories().settings.clone())
        .ok_or_else(|| "STATUS_SURFACE_SETTINGS_UNAVAILABLE".to_string())
}

pub fn set_enabled_with_repository(
    app: &tauri::AppHandle,
    repository: &SettingsRepository,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettings, String> {
    let state = app.state::<Mutex<StatusSurfaceState>>();
    let mut state = state
        .lock()
        .map_err(|_| "STATUS_SURFACE_STATE_UNAVAILABLE".to_string())?;
    let mut runtime = TauriSurfaceRuntime { app, state: &mut state };
    transition(&mut runtime, repository, surface, enabled)
}

pub fn set_enabled_and_emit(
    app: &tauri::AppHandle,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettingsDto, String> {
    let repository = settings_repository(app)?;
    let settings =
        set_enabled_with_repository(app, &repository, surface, enabled)?;
    let dto = AppSettingsDto::from_settings(&settings);
    if app.emit(crate::events::SETTINGS_CHANGED, &dto).is_err() {
        tracing::warn!(
            code = "STATUS_SURFACE_SETTINGS_EVENT_FAILED",
            "status surface settings event was not delivered"
        );
    }
    Ok(dto)
}
~~~

TauriSurfaceRuntime::apply matches the enum once and delegates to TaskbarOverlay::apply_enabled or FloatBall::apply_enabled.

- [ ] **Step 6: Add and register the typed command**

Declare `pub(crate) mod controller;` from status_surfaces.rs and `mod status_surfaces; pub use status_surfaces::*;` from commands/mod.rs.

~~~rust
#[tauri::command]
pub async fn set_status_surface_enabled(
    app: tauri::AppHandle,
    surface: StatusSurfaceKind,
    enabled: bool,
) -> Result<AppSettingsDto, String> {
    crate::status_surfaces::controller::set_enabled_and_emit(
        &app,
        surface,
        enabled,
    )
}
~~~

Export commands::status_surfaces from commands/mod.rs and add commands::set_status_surface_enabled to tauri::generate_handler! in main.rs.

- [ ] **Step 7: Run controller and command tests**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml status_surfaces::controller
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml set_status_surface_enabled
cargo fmt --all
~~~

Expected: PASS.

- [ ] **Step 8: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/status_surfaces apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/commands apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Add status surface controller"
~~~

---

### Task 3: Converge Legacy Settings, Monitor, and Native Close

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/commands/settings.rs:10-65
- Modify: apps/desktop-tauri/src-tauri/src/status_surfaces.rs:18-170
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/mod.rs:15-63
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs:47-94
- Modify: apps/desktop-tauri/src-tauri/src/main.rs:245-274
- Test: apps/desktop-tauri/src-tauri/src/commands/settings.rs:95-165
- Test: apps/desktop-tauri/src-tauri/src/status_surfaces.rs:145-170

**Interfaces:**
- Consumes: controller::transition、StatusSurfaceKind、2 秒 monitor。
- Produces: split_surface_patch(SettingsPatch)；schedule_set_enabled；enabled=false 时也执行 manager reconciliation。

- [ ] **Step 1: Write failing legacy-patch split tests**

~~~rust
#[test]
fn surface_fields_are_removed_before_generic_repository_update() {
    let patch = SettingsPatch {
        theme: Some(ThemePreference::Dark),
        taskbar_status_enabled: Some(false),
        float_ball_enabled: Some(true),
        ..SettingsPatch::default()
    };

    let (base, requested) = split_surface_patch(patch);
    assert_eq!(base.theme, Some(ThemePreference::Dark));
    assert_eq!(base.taskbar_status_enabled, None);
    assert_eq!(base.float_ball_enabled, None);
    assert_eq!(
        requested,
        vec![
            (StatusSurfaceKind::TaskbarStatus, false),
            (StatusSurfaceKind::FloatBall, true),
        ]
    );
}
~~~

- [ ] **Step 2: Write a failing native-window mapping test**

~~~rust
#[test]
fn auxiliary_labels_map_to_permanent_disable_intents() {
    assert_eq!(
        surface_for_window_label("taskbar-status"),
        Some(StatusSurfaceKind::TaskbarStatus)
    );
    assert_eq!(
        surface_for_window_label("float-ball"),
        Some(StatusSurfaceKind::FloatBall)
    );
    assert_eq!(surface_for_window_label("settings"), None);
}

#[test]
fn disabled_surface_reconciliation_selects_cleanup() {
    assert_eq!(
        reconciliation_action(false),
        ReconciliationAction::Cleanup
    );
    assert_eq!(
        reconciliation_action(true),
        ReconciliationAction::Reposition
    );
}
~~~

- [ ] **Step 3: Run RED**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml surface_fields_are_removed
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml auxiliary_labels_map
~~~

Expected: FAIL because split_surface_patch, surface_for_window_label and reconciliation_action do not exist.

- [ ] **Step 4: Route generic update_settings surface fields through the controller**

Implementation order:

~~~rust
fn split_surface_patch(
    mut patch: SettingsPatch,
) -> (SettingsPatch, Vec<(StatusSurfaceKind, bool)>) {
    let mut requested = Vec::new();
    if let Some(enabled) = patch.taskbar_status_enabled.take() {
        requested.push((StatusSurfaceKind::TaskbarStatus, enabled));
    }
    if let Some(enabled) = patch.float_ball_enabled.take() {
        requested.push((StatusSurfaceKind::FloatBall, enabled));
    }
    (patch, requested)
}

let patch = patch.into_patch()?;
let (patch, requested_surfaces) = split_surface_patch(patch);
let repository = settings_repository(&state)?;
let mut settings = if patch == SettingsPatch::default() {
    repository.load().map_err(|_| "SETTINGS_LOAD_FAILED".to_string())?
} else {
    repository.update(patch).map_err(|_| "SETTINGS_SAVE_FAILED".to_string())?
};
for (surface, enabled) in requested_surfaces {
    settings = crate::status_surfaces::controller::set_enabled_with_repository(
        &app,
        &repository,
        surface,
        enabled,
    )?;
}
let dto = AppSettingsDto::from_settings(&settings);
if app.emit(crate::events::SETTINGS_CHANGED, &dto).is_err() {
    tracing::warn!(
        code = "SETTINGS_EVENT_FAILED",
        "settings event was not delivered"
    );
}
Ok(dto)
~~~

Do not persist a surface flag before its window operation succeeds. The active settings UI will move to the typed command in Task 4; this path remains for compatibility only.

- [ ] **Step 5: Make disabled managers reconcile residual windows**

TaskbarOverlay::handle_shell_change and FloatBall::handle_shell_change must follow:

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconciliationAction {
    Reposition,
    Cleanup,
}

fn reconciliation_action(enabled: bool) -> ReconciliationAction {
    if enabled {
        ReconciliationAction::Reposition
    } else {
        ReconciliationAction::Cleanup
    }
}

match reconciliation_action(self.is_enabled()) {
    ReconciliationAction::Reposition => self.reposition(app),
    ReconciliationAction::Cleanup => self.cleanup_disabled_window(app),
}
~~~

Implement the two manager methods explicitly:

~~~rust
// TaskbarOverlay
fn cleanup_disabled_window(
    &mut self,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    crate::status_surfaces::window_lifecycle::close_cached_or_labeled(
        app,
        &mut self.window,
        window::TASKBAR_WINDOW_LABEL,
    )?;
    self.last_slot = None;
    Ok(())
}

// FloatBall
fn cleanup_disabled_window(
    &mut self,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    crate::status_surfaces::window_lifecycle::close_cached_or_labeled(
        app,
        &mut self.window,
        window::FLOAT_BALL_WINDOW_LABEL,
    )?;
    Ok(())
}
~~~

A HiddenPendingDestroy result is successful for the current tick; because the Tauri label still resolves, the next 2-second tick calls destroy again.

- [ ] **Step 6: Route CloseRequested through the same controller**

~~~rust
pub fn surface_for_window_label(
    label: &str,
) -> Option<StatusSurfaceKind> {
    match label {
        crate::taskbar_overlay::window::TASKBAR_WINDOW_LABEL => {
            Some(StatusSurfaceKind::TaskbarStatus)
        }
        crate::float_ball::window::FLOAT_BALL_WINDOW_LABEL => {
            Some(StatusSurfaceKind::FloatBall)
        }
        _ => None,
    }
}

pub fn schedule_set_enabled(
    app: tauri::AppHandle,
    surface: StatusSurfaceKind,
    enabled: bool,
) {
    tauri::async_runtime::spawn(async move {
        if controller::set_enabled_and_emit(&app, surface, enabled).is_err() {
            tracing::warn!(
                code = "STATUS_SURFACE_TRANSITION_FAILED",
                "status surface transition did not complete"
            );
        }
    });
}

if let Some(surface) = status_surfaces::surface_for_window_label(window.label()) {
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            status_surfaces::schedule_set_enabled(
                window.app_handle().clone(),
                surface,
                false,
            );
        }
        // keep Destroyed, Moved and ScaleFactorChanged handling
        _ => {}
    }
    return;
}
~~~

schedule_set_enabled spawns the controller call, emits only stable diagnostic codes, and never logs account data or raw Tauri/storage errors.

- [ ] **Step 7: Run focused and full Tauri tests**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml surface_fields_are_removed
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml auxiliary_labels_map
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo fmt --all
~~~

Expected: PASS.

- [ ] **Step 8: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/commands/settings.rs apps/desktop-tauri/src-tauri/src/status_surfaces.rs apps/desktop-tauri/src-tauri/src/taskbar_overlay apps/desktop-tauri/src-tauri/src/float_ball apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Converge status surface state"
~~~

---

### Task 4: Add the Typed Frontend Toggle Bridge

**Files:**
- Modify: apps/desktop-tauri/src/types/bridge.ts:37-101
- Modify: apps/desktop-tauri/src/types/bridge.test.ts:1-145
- Modify: apps/desktop-tauri/src/lib/tauri.ts:1-137
- Modify: apps/desktop-tauri/src/hooks/useSettings.ts:1-58
- Modify: apps/desktop-tauri/src/hooks/useSettings.test.tsx:1-75
- Modify: apps/desktop-tauri/src/surfaces/Settings.tsx:35-105
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx:1-93
- Modify: apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.test.tsx:1-42

**Interfaces:**
- Consumes: Rust command set_status_surface_enabled(surface, enabled).
- Produces: StatusSurfaceKind = "taskbarStatus" | "floatBall"；setStatusSurfaceEnabled；UseSettingsResult.setSurfaceEnabled。

- [ ] **Step 1: Write failing bridge contract assertions**

~~~ts
expect(commands.setStatusSurfaceEnabled).toBe(
  "set_status_surface_enabled",
);

await setStatusSurfaceEnabled("taskbarStatus", false);
expect(invokeMock).toHaveBeenCalledWith("set_status_surface_enabled", {
  surface: "taskbarStatus",
  enabled: false,
});
~~~

- [ ] **Step 2: Write failing GeneralTab routing assertions**

~~~tsx
const setSurfaceEnabled = vi.fn().mockResolvedValue(defaultAppSettings);
render(
  <GeneralTab
    settings={defaultAppSettings}
    update={vi.fn()}
    setSurfaceEnabled={setSurfaceEnabled}
  />,
);

fireEvent.click(
  screen.getByRole("checkbox", { name: "Show status in taskbar" }),
);
fireEvent.click(
  screen.getByRole("checkbox", { name: "Show floating status ball" }),
);

expect(setSurfaceEnabled).toHaveBeenNthCalledWith(
  1,
  "taskbarStatus",
  true,
);
expect(setSurfaceEnabled).toHaveBeenNthCalledWith(2, "floatBall", true);
~~~

- [ ] **Step 3: Run RED**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/types/bridge.test.ts src/hooks/useSettings.test.tsx src/surfaces/settings/tabs/GeneralTab.test.tsx
~~~

Expected: FAIL because the command name, function and prop do not exist.

- [ ] **Step 4: Implement the typed bridge**

~~~ts
export type StatusSurfaceKind = "taskbarStatus" | "floatBall";

export const commands = {
  // existing names remain unchanged
  setStatusSurfaceEnabled: "set_status_surface_enabled",
} as const;

export const setStatusSurfaceEnabled = (
  surface: StatusSurfaceKind,
  enabled: boolean,
) =>
  invoke<AppSettingsDto>(commands.setStatusSurfaceEnabled, {
    surface,
    enabled,
  });
~~~

- [ ] **Step 5: Expose the operation from useSettings**

~~~ts
export interface UseSettingsResult {
  settings: AppSettingsDto;
  update(patch: SettingsPatchDto): Promise<AppSettingsDto>;
  setSurfaceEnabled(
    surface: StatusSurfaceKind,
    enabled: boolean,
  ): Promise<AppSettingsDto>;
}

const setSurfaceEnabled = useCallback(
  async (surface: StatusSurfaceKind, enabled: boolean) => {
    const next = await invokeSetStatusSurfaceEnabled(surface, enabled);
    setSettings(next);
    return next;
  },
  [],
);
~~~

Settings passes this function to GeneralTab. Non-surface settings continue to use update.

- [ ] **Step 6: Run tests and TypeScript build**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/types/bridge.test.ts src/hooks/useSettings.test.tsx src/surfaces/settings/tabs/GeneralTab.test.tsx
pnpm --dir apps/desktop-tauri run build
~~~

Expected: PASS.

- [ ] **Step 7: Commit**

~~~powershell
git add apps/desktop-tauri/src/types/bridge.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/hooks/useSettings.ts apps/desktop-tauri/src/hooks/useSettings.test.tsx apps/desktop-tauri/src/surfaces/Settings.tsx apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.tsx apps/desktop-tauri/src/surfaces/settings/tabs/GeneralTab.test.tsx
git commit -m "Route status toggles through typed bridge"
~~~

---

### Task 5: Derive a Deterministic Dual-Quota View Model

**Files:**
- Create: apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts
- Create: apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts
- Modify: apps/desktop-tauri/src/hooks/useStatusSurface.ts:1-167
- Modify: apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx:1-150
- Modify: apps/desktop-tauri/src/test/profileUsageFixtures.ts:1-135
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx:1-33
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.tsx:1-120

**Interfaces:**
- Consumes: ProfileSummaryDto、ProfileUsageStateDto、AppSettingsDto["displayMode"]、nowMs。
- Produces: StatusQuotaMetric、StatusSurfaceViewModel、buildStatusSurfaceViewModel(input)；useStatusSurface 返回模型字段和 openPanel/disableSurface 操作。

- [ ] **Step 1: Write failing metric identification and urgency tests**

~~~ts
function modelFor(
  displayMode: "remaining" | "used" = "remaining",
  stateOverride?: ProfileUsageStateDto,
) {
  const bootstrap = readyTwoWindowFixture();
  const profile = {
    ...bootstrap.profiles[0]!,
    accountDisplayName: "Ming Zhao",
    accountStatus: "signedIn" as const,
  };
  return buildStatusSurfaceViewModel({
    profile,
    state: stateOverride ?? bootstrap.usageByProfile.personal!,
    displayMode,
    nowMs: Date.parse("2026-08-06T02:08:00Z"),
  });
}

it("identifies five-hour and weekly metrics by duration", () => {
  const bootstrap = readyTwoWindowFixture();
  const profile = bootstrap.profiles[0]!;
  const state = bootstrap.usageByProfile.personal!;

  const model = buildStatusSurfaceViewModel({
    profile,
    state,
    displayMode: "remaining",
    nowMs: Date.parse("2026-08-06T02:08:00Z"),
  });

  expect(model.primaryMetric).toMatchObject({
    kind: "fiveHour",
    displayedPercent: 42,
  });
  expect(model.secondaryMetric).toMatchObject({
    kind: "weekly",
    displayedPercent: 61,
  });
  expect(model.urgentMetric?.kind).toBe("fiveHour");
  expect(model.primaryMetric?.resetText).toBe("2h52m");
});
~~~

Change readyTwoWindowFixture so its weekly remainingPercent is 61, matching the approved visual fixture. Add a tie test where both remaining values are 42 and urgentMetric.kind must be fiveHour. Add a test where an unknown 1,440-minute window is ignored rather than labeled weekly.

~~~ts
it("uses five-hour as the deterministic urgency tie-breaker", () => {
  const bootstrap = readyTwoWindowFixture();
  const state = bootstrap.usageByProfile.personal!;
  state.secondary = usageWindow(42, {
    limitId: "weekly",
    windowDurationMinutes: 10_080,
  });
  expect(modelFor("remaining", state).urgentMetric?.kind).toBe("fiveHour");
});

it("does not relabel an unknown duration", () => {
  const state = profileUsageFixture("personal", 42);
  state.primary = usageWindow(42, {
    limitId: "daily",
    windowDurationMinutes: 1_440,
  });
  state.secondary = null;
  const model = modelFor("remaining", state);
  expect(model.primaryMetric).toBeNull();
  expect(model.secondaryMetric).toBeNull();
});
~~~

- [ ] **Step 2: Write failing state and display-mode tests**

~~~ts
it("changes displayed values without changing urgency", () => {
  const remaining = modelFor("remaining");
  const used = modelFor("used");
  expect(remaining.urgentMetric?.kind).toBe("fiveHour");
  expect(used.urgentMetric?.kind).toBe("fiveHour");
  expect(remaining.primaryMetric?.displayedPercent).toBe(42);
  expect(used.primaryMetric?.displayedPercent).toBe(58);
});

it("preserves identity and cached quota when stale", () => {
  const stale = staleOfflineFixture();
  const model = modelFor("remaining", stale.usageByProfile.personal!);
  expect(model.displayName).toBe("Ming Zhao");
  expect(model.primaryMetric?.displayedPercent).toBe(42);
  expect(model.status).toBe("critical");
  expect(model.freshness).toBe("stale");
});

it("never invents zero for missing usage", () => {
  const ready = readyTwoWindowFixture().usageByProfile.personal!;
  const model = modelFor("remaining", {
    ...ready,
    primary: null,
    secondary: null,
    additionalWindows: [],
    freshness: "missing",
  });
  expect(model.urgentMetric).toBeNull();
  expect(model.status).toBe("missing");
  expect(model.refreshStatus).toBe("等待数据");
});
~~~

- [ ] **Step 3: Run RED**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/lib/statusSurfaceViewModel.test.ts
~~~

Expected: FAIL because the view-model module is absent.

- [ ] **Step 4: Implement the exact public types**

~~~ts
export type StatusSurfaceStatus =
  | "ready"
  | "warning"
  | "critical"
  | "refreshing"
  | "stale"
  | "missing";

export interface StatusQuotaMetric {
  kind: "fiveHour" | "weekly";
  label: "5H" | "周";
  usedPercent: number;
  remainingPercent: number;
  displayedPercent: number;
  displayMode: "remaining" | "used";
  resetText: string;
  resetsAt: string | null;
}

export interface StatusSurfaceViewModel {
  displayName: string;
  accountStatus: "signedIn" | "signedOut" | "unavailable";
  primaryMetric: StatusQuotaMetric | null;
  secondaryMetric: StatusQuotaMetric | null;
  urgentMetric: StatusQuotaMetric | null;
  freshness: "fresh" | "stale" | "missing";
  refreshStatus: string;
  updatedText: string | null;
  status: StatusSurfaceStatus;
}
~~~

- [ ] **Step 5: Implement pure derivation helpers**

Use constants FIVE_HOUR_MINUTES = 300 and WEEKLY_MINUTES = 10_080. Search state.primary, state.secondary, then state.additionalWindows; accept only exact duration matches.

Move cleanIdentity, profileDisplayName and accountStatusFallback from useStatusSurface.ts into statusSurfaceViewModel.ts so identity derivation is pure and cannot form a hook/lib import cycle. Export profileDisplayName from the new module and update the existing hook test import.

~~~ts
function toMetric(
  kind: StatusQuotaMetric["kind"],
  window: UsageWindowDto,
  displayMode: AppSettingsDto["displayMode"],
  nowMs: number,
): StatusQuotaMetric {
  return {
    kind,
    label: kind === "fiveHour" ? "5H" : "周",
    usedPercent: clampPercent(window.usedPercent),
    remainingPercent: clampPercent(window.remainingPercent),
    displayedPercent: clampPercent(
      displayMode === "used"
        ? window.usedPercent
        : window.remainingPercent,
    ),
    displayMode,
    resetText: formatResetCountdown(window.resetsAt, nowMs),
    resetsAt: window.resetsAt,
  };
}

function urgentMetric(
  fiveHour: StatusQuotaMetric | null,
  weekly: StatusQuotaMetric | null,
): StatusQuotaMetric | null {
  if (!fiveHour) return weekly;
  if (!weekly) return fiveHour;
  return fiveHour.remainingPercent <= weekly.remainingPercent
    ? fiveHour
    : weekly;
}
~~~

formatResetCountdown returns — for invalid/missing time, 即将重置 when delta <= 0, Xm under one hour, XhYYm under 48 hours, and X天 after that. updatedText uses the same injected nowMs so tests never depend on wall-clock time.

- [ ] **Step 6: Update useStatusSurface without duplicating derivation**

~~~ts
const model = useMemo(
  () =>
    buildStatusSurfaceViewModel({
      profile,
      state: usage.state,
      displayMode:
        bootstrap?.settings.displayMode ?? EMPTY_BOOTSTRAP.settings.displayMode,
      nowMs: Date.now(),
    }),
  [bootstrap?.settings.displayMode, profile, usage.state],
);

const disableSurface = useCallback(
  (surface: StatusSurfaceKind) =>
    setStatusSurfaceEnabled(surface, false),
  [],
);

return {
  ...model,
  bootstrap,
  profile,
  state: usage.state,
  isDragging,
  setIsDragging,
  openPanel,
  disableSurface,
};
~~~

Temporarily update the existing taskbar and float-ball numeric reads to surface.urgentMetric?.displayedPercent so the branch compiles before their visual rewrites.

- [ ] **Step 7: Run model, hook, and frontend build**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/lib/statusSurfaceViewModel.test.ts src/hooks/useStatusSurface.test.tsx
pnpm --dir apps/desktop-tauri run build
~~~

Expected: PASS.

- [ ] **Step 8: Commit**

~~~powershell
git add apps/desktop-tauri/src/lib/statusSurfaceViewModel.ts apps/desktop-tauri/src/lib/statusSurfaceViewModel.test.ts apps/desktop-tauri/src/hooks/useStatusSurface.ts apps/desktop-tauri/src/hooks/useStatusSurface.test.tsx apps/desktop-tauri/src/test/profileUsageFixtures.ts apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx apps/desktop-tauri/src/surfaces/FloatBall.tsx
git commit -m "Derive dual quota status model"
~~~

---

### Task 6: Build the Glass Orbit Taskbar Capsule

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs:4-66
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx:1-33
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.css:1-95
- Modify: apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx:1-40

**Interfaces:**
- Consumes: useStatusSurface dual metrics、disableSurface("taskbarStatus")、openPanel()。
- Produces: 318px taskbar window contract；two sibling buttons；close failure inline status。

- [ ] **Step 1: Write failing information hierarchy tests**

~~~tsx
const bootstrap = readyTwoWindowFixture();
bootstrap.profiles[0]!.accountDisplayName = "Ming Zhao";
invokeMock.mockImplementation(async (command: string) => {
  if (command === "get_bootstrap_state") return bootstrap;
  if (command === "set_status_surface_enabled") {
    return { ...bootstrap.settings, taskbarStatusEnabled: false };
  }
  return undefined;
});

render(<TaskbarStatus />);

expect(
  await screen.findByRole("button", { name: /打开完整面板.*Ming Zhao/ }),
).toBeInTheDocument();
expect(screen.getByText(/5H 42%/)).toBeInTheDocument();
expect(screen.getByText(/周 61%/)).toBeInTheDocument();
expect(screen.getByText(/2h52m/)).toBeInTheDocument();
expect(
  screen.getByRole("button", { name: "关闭任务栏状态" }),
).toBeInTheDocument();
~~~

- [ ] **Step 2: Write failing event-isolation and error tests**

~~~tsx
const close = await screen.findByRole("button", {
  name: "关闭任务栏状态",
});
fireEvent.click(close);
await waitFor(() =>
  expect(invokeMock).toHaveBeenCalledWith(
    "set_status_surface_enabled",
    { surface: "taskbarStatus", enabled: false },
  ),
);
expect(invokeMock).not.toHaveBeenCalledWith("open_tray_panel");

for (const button of screen.getAllByRole("button")) {
  expect(button.querySelector("button")).toBeNull();
}
~~~

For failure, reject only set_status_surface_enabled and assert role=status contains 关闭失败，请重试 while the close button remains enabled:

~~~tsx
invokeMock.mockImplementation(async (command: string) => {
  if (command === "get_bootstrap_state") return readyTwoWindowFixture();
  if (command === "set_status_surface_enabled") {
    throw new Error("STATUS_SURFACE_WINDOW_CLOSE_FAILED");
  }
  return undefined;
});
render(<TaskbarStatus />);
fireEvent.click(
  await screen.findByRole("button", { name: "关闭任务栏状态" }),
);
expect(await screen.findByRole("status")).toHaveTextContent(
  "关闭失败，请重试",
);
expect(
  screen.getByRole("button", { name: "关闭任务栏状态" }),
).toBeEnabled();
~~~

- [ ] **Step 3: Run RED**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/surfaces/TaskbarStatus.test.tsx
~~~

Expected: FAIL because the second quota and independent close button are absent.

- [ ] **Step 4: Implement the sibling-button DOM**

~~~tsx
function initials(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "C";
  return parts.slice(0, 2).map((part) => part[0]!.toUpperCase()).join("");
}

function metricValue(metric: StatusQuotaMetric | null): string {
  return metric ? `${metric.displayedPercent}%` : "—";
}

function compactMetric(
  metric: StatusQuotaMetric | null,
  fallbackLabel: "5H" | "周",
): string {
  return `${metric?.label ?? fallbackLabel} ${metricValue(metric)}`;
}

function nearestReset(surface: UseStatusSurfaceResult): string {
  const candidate = [surface.primaryMetric, surface.secondaryMetric]
    .filter(
      (metric): metric is StatusQuotaMetric =>
        metric !== null &&
        metric.resetsAt !== null &&
        Number.isFinite(Date.parse(metric.resetsAt)),
    )
    .sort(
      (left, right) =>
        Date.parse(left.resetsAt!) - Date.parse(right.resetsAt!),
    )[0];
  return candidate?.resetText ?? "—";
}

function mainAriaLabel(surface: UseStatusSurfaceResult): string {
  return [
    "打开完整面板",
    surface.displayName,
    compactMetric(surface.primaryMetric, "5H"),
    compactMetric(surface.secondaryMetric, "周"),
    surface.status,
  ].join("，");
}

const [closeError, setCloseError] = useState<string | null>(null);
const closeSurface = async (
  event: React.MouseEvent<HTMLButtonElement>,
) => {
  event.stopPropagation();
  setCloseError(null);
  try {
    await surface.disableSurface("taskbarStatus");
  } catch {
    setCloseError("关闭失败，请重试");
  }
};

return (
  <div
    className="taskbar-status"
    data-status={surface.status}
    data-freshness={surface.freshness}
  >
    <button
      type="button"
      className="taskbar-status__main"
      aria-label={mainAriaLabel(surface)}
      onClick={() => void surface.openPanel()}
    >
      <span className="taskbar-status__avatar" aria-hidden="true">
        {initials(surface.displayName)}
        <span className="taskbar-status__state-dot" />
      </span>
      <span className="taskbar-status__copy">
        <span className="taskbar-status__identity">
          {surface.displayName}
        </span>
        {closeError ? (
          <span className="taskbar-status__error" role="status">
            {closeError}
          </span>
        ) : (
          <span className="taskbar-status__quotas">
            {compactMetric(surface.primaryMetric, "5H")}
            <span aria-hidden="true"> · </span>
            {compactMetric(surface.secondaryMetric, "周")}
            <span className="taskbar-status__reset">
              {" · "}{nearestReset(surface)} 重置
            </span>
          </span>
        )}
      </span>
      <span className="taskbar-status__urgent">
        <span className="taskbar-status__urgent-label">
          {surface.urgentMetric?.label ?? "—"}
        </span>
        <span className="taskbar-status__urgent-value">
          {metricValue(surface.urgentMetric)}
        </span>
      </span>
    </button>
    <button
      type="button"
      className="taskbar-status__close"
      aria-label="关闭任务栏状态"
      title="关闭任务栏状态"
      onClick={closeSurface}
    >
      <span aria-hidden="true">×</span>
    </button>
  </div>
);
~~~

closeSurface stops propagation, awaits disableSurface, and sets exactly 关闭失败，请重试 on rejection.

- [ ] **Step 5: Implement the approved Glass Orbit CSS**

Use these fixed tokens:

~~~css
.taskbar-status {
  --glass: rgba(24, 26, 34, 0.86);
  --glass-hover: rgba(34, 37, 48, 0.94);
  --border: rgba(255, 255, 255, 0.13);
  --primary: #7b8cff;
  --secondary: #67d49a;
  width: 100%;
  height: 100%;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 30px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: linear-gradient(135deg, rgba(46, 50, 66, 0.92), var(--glass));
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28),
              inset 0 1px rgba(255, 255, 255, 0.08);
  container-type: inline-size;
}

.taskbar-status__main {
  min-width: 0;
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
}

.taskbar-status__close {
  width: 27px;
  height: 27px;
  align-self: center;
  border-radius: 50%;
}
~~~

taskbar-status__state-dot is a 6px green dot anchored to the avatar's lower-right corner; refreshing breathes this dot without rotating the card. taskbar-status__urgent is a two-row mini stack so the main number is always explicitly labeled 5H or 周.

At max-width 286px hide taskbar-status__identity; at max-width 250px hide taskbar-status__reset. Never hide both metrics, urgent number/label, or close button. warning uses #e7ad5b, critical uses #ef7c88, stale/missing uses #9299aa. reduced-motion removes pulse/translate transitions.

- [ ] **Step 6: Update the native width contract**

~~~rust
pub const TASKBAR_LOGICAL_WIDTH: u32 = 318;
~~~

Update overlay_label_and_frontend_route_are_stable to assert 318.

- [ ] **Step 7: Run UI, Rust contract, and build**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/surfaces/TaskbarStatus.test.tsx
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml overlay_label_and_frontend_route_are_stable
pnpm --dir apps/desktop-tauri run build
~~~

Expected: PASS.

- [ ] **Step 8: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/taskbar_overlay/window.rs apps/desktop-tauri/src/surfaces/TaskbarStatus.tsx apps/desktop-tauri/src/surfaces/TaskbarStatus.css apps/desktop-tauri/src/surfaces/TaskbarStatus.test.tsx
git commit -m "Redesign taskbar status capsule"
~~~

---

### Task 7: Add Anchored Expanded Float-Ball Geometry

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs:1-319
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs:1-252
- Modify: apps/desktop-tauri/src-tauri/src/float_ball/window.rs:1-86
- Modify: apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs
- Modify: apps/desktop-tauri/src-tauri/src/main.rs:115-141
- Test: apps/desktop-tauri/src-tauri/src/float_ball/geometry.rs
- Test: apps/desktop-tauri/src-tauri/src/float_ball/mod.rs

**Interfaces:**
- Consumes: saved collapsed logical Point、MonitorGeometry、current work area。
- Produces: FloatBallPresentation::{Collapsed, Expanded}；presentation_rect；FloatBall::set_expanded(app, expanded)；set_float_ball_expanded(expanded) command。

- [ ] **Step 1: Write failing size and edge-anchor tests**

~~~rust
#[test]
fn collapsed_size_is_eighty_eight_logical_pixels() {
    assert_eq!(FLOAT_BALL_COLLAPSED_WIDTH, 88);
    assert_eq!(FLOAT_BALL_COLLAPSED_HEIGHT, 88);
}

#[test]
fn bottom_right_ball_expands_up_and_left() {
    let work_area = Rect { x: 0, y: 0, width: 1920, height: 1032 };
    let collapsed = Point { x: 1824, y: 936 };
    assert_eq!(
        presentation_rect(
            collapsed,
            work_area,
            1.0,
            FloatBallPresentation::Expanded,
        ),
        Rect { x: 1652, y: 876, width: 260, height: 148 }
    );
}

#[test]
fn top_left_ball_expands_down_and_right() {
    let work_area = Rect { x: -1920, y: -120, width: 1920, height: 1032 };
    let collapsed = Point { x: -1912, y: -112 };
    let rect = presentation_rect(
        collapsed,
        work_area,
        1.0,
        FloatBallPresentation::Expanded,
    );
    assert_eq!(rect, Rect { x: -1912, y: -112, width: 260, height: 148 });
}
~~~

Add table-driven assertions for 1.0, 1.5 and 2.0 scales that every edge remains inside work_area and that negative monitor origins remain negative.

~~~rust
#[test]
fn expanded_rect_is_inside_work_area_at_supported_scales() {
    for scale in [1.0, 1.5, 2.0] {
        let work_area = Rect {
            x: -2560,
            y: -120,
            width: 2560,
            height: 1368,
        };
        let collapsed = initial_position(work_area, work_area, scale);
        let rect = presentation_rect(
            collapsed,
            work_area,
            scale,
            FloatBallPresentation::Expanded,
        );
        assert!(rect.x >= work_area.x);
        assert!(rect.y >= work_area.y);
        assert!(rect.x + rect.width <= work_area.x + work_area.width);
        assert!(rect.y + rect.height <= work_area.y + work_area.height);
        assert!(rect.x < 0);
    }
}
~~~

- [ ] **Step 2: Write a failing expanded-move preservation test**

~~~rust
#[test]
fn expanded_presentation_never_replaces_saved_collapsed_position() {
    let mut state = FloatBallState::default();
    state.remember_logical_position(Point { x: -240, y: 96 });
    state.set_presentation(FloatBallPresentation::Expanded);
    assert!(!state.should_persist_moved_event());
    assert_eq!(state.logical_position(), Some(Point { x: -240, y: 96 }));
}
~~~

- [ ] **Step 3: Run RED**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball::geometry
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml expanded_presentation
~~~

Expected: FAIL because the new constants, presentation enum and rect function are absent.

- [ ] **Step 4: Implement presentation constants and geometry**

~~~rust
pub const FLOAT_BALL_COLLAPSED_WIDTH: i32 = 88;
pub const FLOAT_BALL_COLLAPSED_HEIGHT: i32 = 88;
pub const FLOAT_BALL_EXPANDED_WIDTH: i32 = 260;
pub const FLOAT_BALL_EXPANDED_HEIGHT: i32 = 148;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FloatBallPresentation {
    #[default]
    Collapsed,
    Expanded,
}
~~~

presentation_rect scales all four dimensions, chooses the nearer horizontal and vertical work-area edges, preserves that edge while expanding, and clamps the final Rect with an 8-logical-pixel scaled margin.

~~~rust
fn clamp_rect(rect: Rect, work_area: Rect, margin: i32) -> Rect {
    let margin = margin.max(0);
    let min_x = work_area.x.saturating_add(margin);
    let min_y = work_area.y.saturating_add(margin);
    let max_x = work_area
        .x
        .saturating_add(work_area.width)
        .saturating_sub(rect.width)
        .saturating_sub(margin)
        .max(min_x);
    let max_y = work_area
        .y
        .saturating_add(work_area.height)
        .saturating_sub(rect.height)
        .saturating_sub(margin)
        .max(min_y);
    Rect {
        x: rect.x.clamp(min_x, max_x),
        y: rect.y.clamp(min_y, max_y),
        width: rect.width,
        height: rect.height,
    }
}

let anchor_right = distance_right <= distance_left;
let anchor_bottom = distance_bottom <= distance_top;
let x = if anchor_right {
    collapsed.x + collapsed_width - width
} else {
    collapsed.x
};
let y = if anchor_bottom {
    collapsed.y + collapsed_height - height
} else {
    collapsed.y
};
clamp_rect(Rect { x, y, width, height }, work_area, margin)
~~~

- [ ] **Step 5: Store presentation separately from collapsed geometry**

FloatBallState gains presentation. set_expanded changes presentation, calls reposition, and restores the old presentation when reposition fails. handle_moved returns immediately while presentation is Expanded; dragging always begins after the frontend requests collapse.

~~~rust
pub fn set_expanded(
    &mut self,
    app: &tauri::AppHandle,
    expanded: bool,
) -> Result<(), String> {
    if !self.state.enabled {
        return Err("FLOAT_BALL_DISABLED".to_string());
    }
    let previous = self.state.presentation();
    self.state.set_presentation(if expanded {
        FloatBallPresentation::Expanded
    } else {
        FloatBallPresentation::Collapsed
    });
    if let Err(error) = self.reposition(app) {
        self.state.set_presentation(previous);
        return Err(error);
    }
    Ok(())
}
~~~

- [ ] **Step 6: Apply size and position as one window update**

Change window::position_and_show to accept Rect and pass width/height together to shell::dwm::set_no_activate_bounds. The builder initial size becomes 88 × 88 and still pins Theme::Dark.

- [ ] **Step 7: Add and register the expand command**

~~~rust
#[tauri::command]
pub async fn set_float_ball_expanded(
    app: tauri::AppHandle,
    expanded: bool,
) -> Result<(), String> {
    crate::status_surfaces::set_float_ball_expanded(&app, expanded)
}
~~~

Register commands::set_float_ball_expanded in main.rs.

- [ ] **Step 8: Run geometry, lifecycle, and complete Tauri tests**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml float_ball::geometry
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml expanded_presentation
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo fmt --all
~~~

Expected: PASS.

- [ ] **Step 9: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/float_ball apps/desktop-tauri/src-tauri/src/commands/status_surfaces.rs apps/desktop-tauri/src-tauri/src/main.rs
git commit -m "Add expandable float ball geometry"
~~~

---

### Task 8: Build the Expandable Glass Orbit Float Ball

**Files:**
- Create: apps/desktop-tauri/src/hooks/useFloatBallExpansion.ts
- Create: apps/desktop-tauri/src/hooks/useFloatBallExpansion.test.tsx
- Modify: apps/desktop-tauri/src/lib/tauri.ts:1-137
- Modify: apps/desktop-tauri/src/types/bridge.test.ts
- Modify: apps/desktop-tauri/src/hooks/useStatusSurface.ts
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.tsx:1-120
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.css:1-121
- Modify: apps/desktop-tauri/src/surfaces/FloatBall.test.tsx:1-105

**Interfaces:**
- Consumes: set_float_ball_expanded、disableSurface("floatBall")、dual-quota view model、getCurrentWindow().startDragging()。
- Produces: useFloatBallExpansion；collapsed/expanded Glass Orbit DOM；drag-safe body and permanent X。

- [ ] **Step 1: Write failing typed expand bridge test**

~~~ts
await setFloatBallExpanded(true);
expect(invokeMock).toHaveBeenCalledWith("set_float_ball_expanded", {
  expanded: true,
});
~~~

- [ ] **Step 2: Write failing fake-timer expansion tests**

~~~tsx
vi.useFakeTimers();
const onExpandedChange = vi.fn().mockResolvedValue(undefined);
const { result, unmount } = renderHook(() =>
  useFloatBallExpansion({ onExpandedChange }),
);

act(() => result.current.pointerEntered());
await act(async () => {
  await vi.advanceTimersByTimeAsync(179);
});
expect(onExpandedChange).not.toHaveBeenCalled();
await act(async () => {
  await vi.advanceTimersByTimeAsync(1);
});
expect(onExpandedChange).toHaveBeenCalledWith(true);

act(() => result.current.pointerLeft());
act(() => result.current.pointerEntered());
await vi.runAllTimersAsync();
expect(onExpandedChange).not.toHaveBeenCalledWith(false);

unmount();
expect(vi.getTimerCount()).toBe(0);
~~~

- [ ] **Step 3: Write failing FloatBall interaction tests**

Required assertions:

~~~tsx
expect(
  await screen.findByRole("button", { name: /打开完整面板.*Ming Zhao/ }),
).toBeInTheDocument();
expect(
  screen.getByRole("button", { name: "关闭悬浮球" }),
).toBeInTheDocument();

fireEvent.pointerEnter(screen.getByTestId("float-ball-shell"));
await vi.advanceTimersByTimeAsync(180);
expect(screen.getByText("5小时额度")).toBeInTheDocument();
expect(screen.getByText("每周额度")).toBeInTheDocument();
expect(screen.getByText(/最后更新/)).toBeInTheDocument();
~~~

Keep the existing drag threshold tests. Add assertions that drag first invokes set_float_ball_expanded with false, then startDragging, and never invokes open_tray_panel. Clicking X invokes set_status_surface_enabled with floatBall/false and never bubbles to open_tray_panel.

~~~tsx
invokeMock.mockImplementation(async (command: string) => {
  if (command === "get_bootstrap_state") return readyTwoWindowFixture();
  if (command === "set_status_surface_enabled") {
    throw new Error("STATUS_SURFACE_WINDOW_CLOSE_FAILED");
  }
  return undefined;
});
render(<FloatBall />);
fireEvent.click(
  await screen.findByRole("button", { name: "关闭悬浮球" }),
);
expect(await screen.findByRole("status")).toHaveTextContent(
  "关闭失败，请重试",
);
expect(
  screen.getByRole("button", { name: "关闭悬浮球" }),
).toBeEnabled();
~~~

- [ ] **Step 4: Run RED**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useFloatBallExpansion.test.tsx src/surfaces/FloatBall.test.tsx src/types/bridge.test.ts
~~~

Expected: FAIL because the expansion hook, command and expanded card are absent.

- [ ] **Step 5: Implement the expansion hook**

~~~ts
const EXPAND_DELAY_MS = 180;
const COLLAPSE_DELAY_MS = 120;

export function useFloatBallExpansion({
  onExpandedChange,
}: {
  onExpandedChange(expanded: boolean): Promise<void>;
}) {
  const [expanded, setExpanded] = useState(false);
  const [expansionError, setExpansionError] = useState<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const cancelTimer = useCallback(() => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }, []);

  const request = useCallback(
    async (next: boolean): Promise<boolean> => {
      try {
        await onExpandedChange(next);
        setExpanded(next);
        setExpansionError(null);
        return true;
      } catch {
        setExpansionError("悬浮球尺寸切换失败");
        return false;
      }
    },
    [onExpandedChange],
  );

  const pointerEntered = useCallback(() => {
    cancelTimer();
    timer.current = setTimeout(() => {
      timer.current = null;
      void request(true);
    }, EXPAND_DELAY_MS);
  }, [cancelTimer, request]);

  const pointerLeft = useCallback(() => {
    cancelTimer();
    timer.current = setTimeout(() => {
      timer.current = null;
      void request(false);
    }, COLLAPSE_DELAY_MS);
  }, [cancelTimer, request]);

  const collapseNow = useCallback(() => {
    cancelTimer();
    return request(false);
  }, [cancelTimer, request]);

  useEffect(() => cancelTimer, [cancelTimer]);
  return {
    expanded,
    expansionError,
    pointerEntered,
    pointerLeft,
    collapseNow,
    cancelPending: cancelTimer,
  };
}
~~~

- [ ] **Step 6: Add the frontend command and surface operation**

~~~ts
export const setFloatBallExpanded = (expanded: boolean) =>
  invoke<void>(commands.setFloatBallExpanded, { expanded });
~~~

Add setFloatBallExpanded: "set_float_ball_expanded" to commands. useStatusSurface exposes setFloatBallExpanded without changing the quota model.

- [ ] **Step 7: Implement separate body and close targets**

The root is a non-button div with data-testid=float-ball-shell and pointer enter/leave handlers. Its children are:

~~~text
float-ball-shell
├── body button
│   ├── collapsed orbit
│   └── expanded quota card
├── close button
└── role=status error text
~~~

The body owns pointer capture and drag threshold. On threshold crossing it awaits collapseNow(), sets dragging, then calls startDragging. Pointer-up below threshold opens the panel once; compatibility click is consumed exactly as in the current implementation.

The close target uses this exact failure path:

~~~tsx
const [closeError, setCloseError] = useState<string | null>(null);
const closeSurface = async (
  event: React.MouseEvent<HTMLButtonElement>,
) => {
  event.stopPropagation();
  expansion.cancelPending();
  setCloseError(null);
  try {
    await surface.disableSurface("floatBall");
  } catch {
    setCloseError("关闭失败，请重试");
  }
};
~~~

- [ ] **Step 8: Implement the approved visual states**

Collapsed tokens:

~~~css
.float-ball-shell {
  width: 100%;
  height: 100%;
  position: relative;
  color: #f7f8ff;
}

.float-ball--collapsed {
  width: 88px;
  height: 88px;
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 50%;
  background: radial-gradient(circle at 35% 25%,
              rgba(67, 73, 98, 0.96),
              rgba(20, 22, 30, 0.94) 68%);
  box-shadow: 0 14px 34px rgba(0, 0, 0, 0.38),
              inset 0 1px rgba(255, 255, 255, 0.1);
}
~~~

The orbit is a 5px rounded SVG progress ring using #7b8cff ready, #e7ad5b warning, #ef7c88 critical, #9299aa stale/missing. Center displays the urgent integer; footer displays 5H 剩余 or 周 剩余. The independent 27px X remains visible in the upper-right.

Expanded card:

~~~css
.float-ball--expanded {
  width: 260px;
  height: 148px;
  border: 1px solid rgba(255, 255, 255, 0.13);
  border-radius: 22px;
  background: linear-gradient(145deg,
              rgba(45, 49, 65, 0.96),
              rgba(20, 22, 30, 0.94));
  box-shadow: 0 18px 46px rgba(0, 0, 0, 0.42),
              inset 0 1px rgba(255, 255, 255, 0.08);
}
~~~

The card shows avatar/name/X, side-by-side 5-hour and weekly tracks/reset text, then updatedText and 打开完整面板. Animation duration is 180ms with opacity/scale/translate only. refreshing breathes the status point/track and never spins the whole card. prefers-reduced-motion removes looping and positional animation.

- [ ] **Step 9: Run hook, component, bridge, and build**

~~~powershell
pnpm --dir apps/desktop-tauri test -- src/hooks/useFloatBallExpansion.test.tsx src/surfaces/FloatBall.test.tsx src/types/bridge.test.ts
pnpm --dir apps/desktop-tauri run build
~~~

Expected: PASS.

- [ ] **Step 10: Commit**

~~~powershell
git add apps/desktop-tauri/src/hooks/useFloatBallExpansion.ts apps/desktop-tauri/src/hooks/useFloatBallExpansion.test.tsx apps/desktop-tauri/src/lib/tauri.ts apps/desktop-tauri/src/types/bridge.test.ts apps/desktop-tauri/src/hooks/useStatusSurface.ts apps/desktop-tauri/src/surfaces/FloatBall.tsx apps/desktop-tauri/src/surfaces/FloatBall.css apps/desktop-tauri/src/surfaces/FloatBall.test.tsx
git commit -m "Redesign expandable float ball"
~~~

---

### Task 9: Add Deterministic Proof Scenarios and Complete Windows Verification

**Files:**
- Modify: apps/desktop-tauri/src-tauri/src/proof_harness.rs
- Modify: docs/WINDOWS_PROOF.md
- Create: docs/verification/windows/2026-08-10/cua-observations.md
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-ready.png
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-warning.png
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-critical.png
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-refreshing.png
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-stale.png
- Create: docs/verification/windows/2026-08-10/screenshots/taskbar-missing.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-ready-collapsed.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-ready-expanded.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-warning.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-critical.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-refreshing.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-stale.png
- Create: docs/verification/windows/2026-08-10/screenshots/float-missing.png
- Test: apps/desktop-tauri/src-tauri/src/proof_harness.rs

**Interfaces:**
- Consumes: CODEXBAR_PROOF_MODE、synthetic_bootstrap、fresh debug binary、CUA driver。
- Produces: credential-free taskbar-status:{state} and float-ball:{state} scenarios；source-backed verification record。

- [ ] **Step 1: Write failing proof parser coverage**

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusProofState {
    Ready,
    Warning,
    Critical,
    Refreshing,
    Stale,
    Missing,
}

#[test]
fn every_status_surface_proof_state_parses() {
    for state in ["ready", "warning", "critical", "refreshing", "stale", "missing"] {
        assert!(ProofScenario::parse(&format!("taskbar-status:{state}")).is_some());
        assert!(ProofScenario::parse(&format!("float-ball:{state}")).is_some());
    }
}
~~~

Keep taskbar-status and float-ball as aliases for their ready states.

- [ ] **Step 2: Write failing synthetic-state assertions**

~~~rust
#[test]
fn status_proof_payloads_cover_visual_states_without_credentials() {
    let warning = synthetic_bootstrap(
        ProofScenario::TaskbarStatus(StatusProofState::Warning),
    );
    let warning_usage = warning.usage_by_profile.values().next().unwrap();
    assert_eq!(warning_usage.primary.as_ref().unwrap().used_percent, 78.0);

    let missing = synthetic_bootstrap(
        ProofScenario::FloatBall(StatusProofState::Missing),
    );
    let missing_usage = missing.usage_by_profile.values().next().unwrap();
    assert!(missing_usage.primary.is_none());
    assert!(missing_usage.secondary.is_none());

    let encoded = serde_json::to_string(&warning).unwrap();
    assert!(!encoded.contains("token"));
    assert!(!encoded.contains("cookie"));
}
~~~

- [ ] **Step 3: Run RED**

~~~powershell
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml proof_harness
~~~

Expected: FAIL because parameterized status-surface scenarios are absent.

- [ ] **Step 4: Implement deterministic status states**

Use these exact synthetic states:

| State | primary used | weekly used | refresh status | freshness | error |
|---|---:|---:|---|---|---|
| ready | 58 | 39 | idle | fresh | none |
| warning | 78 | 39 | idle | fresh | none |
| critical | 94 | 39 | idle | fresh | none |
| refreshing | 58 | 39 | refreshing | fresh | none |
| stale | 58 | 39 | idle | stale | offlineOrTimeout |
| missing | no snapshot | no snapshot | idle | missing | none |

ProofScenario becomes TaskbarStatus(StatusProofState) or FloatBall(StatusProofState). Update parse, name, surface, settings_tab, activation and synthetic_account_data exhaustively; preserve all existing tray/settings proof names.

- [ ] **Step 5: Update proof documentation**

docs/WINDOWS_PROOF.md must state:

- taskbar-status:{ready|warning|critical|refreshing|stale|missing}
- float-ball:{ready|warning|critical|refreshing|stale|missing}
- taskbar overlay target width 318 logical pixels.
- float ball is 88 × 88 collapsed and 260 × 148 expanded.
- Close X verification is done outside proof mode against persisted settings, because proof mode intentionally reactivates its requested surface on every launch.

- [ ] **Step 6: Run the full local validation slice**

~~~powershell
cargo fmt --all
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
~~~

Expected: boundary guard, fmt, both clippy invocations with -D warnings, shared Rust tests, Tauri tests, frontend tests, and frontend build all PASS.

- [ ] **Step 7: Produce a fresh debug desktop binary**

First resolve and stop only codex-barbar instances:

~~~powershell
$running = Get-Process -Name codex-barbar -ErrorAction SilentlyContinue
if ($running) { $running | Stop-Process -Force }
pnpm --dir apps/desktop-tauri run tauri:build:debug
Get-Item .\target\debug\codex-barbar.exe |
  Select-Object FullName, Length, LastWriteTime
~~~

Expected: target/debug/codex-barbar.exe has a LastWriteTime after the final source commit.

- [ ] **Step 8: Capture CUA visual states**

For each proof scenario, stop the prior instance, set CODEXBAR_PROOF_MODE, launch the fresh binary hidden, list windows, capture the named window, and save the screenshot under docs/verification/windows/2026-08-10/screenshots.

~~~powershell
$cua = Join-Path $env:LOCALAPPDATA 'Programs\Cua\cua-driver\bin\cua-driver.exe'
$env:CODEXBAR_PROOF_MODE = 'float-ball:ready'
Start-Process -FilePath '.\target\debug\codex-barbar.exe' -WindowStyle Hidden
& $cua call list_windows '{}'
~~~

Required observable assertions:

- Taskbar card stays between task list and notification area and remains fully inside the taskbar rectangle.
- Taskbar displays account, 5H, 周, reset, urgent value and X.
- Float collapsed is 88 × 88 and its X is clickable.
- Hover for less than 180ms stays collapsed; at 180ms it becomes a 260 × 148 card.
- Expanded card stays inside work area near all currently testable edges.
- Body click opens the main panel; drag does not.
- Dark theme remains dark under settings theme=system after float ball opens.

- [ ] **Step 9: Verify permanent close and restart convergence outside proof mode**

Use the settings UI to enable one surface, then:

1. Record its visible window with CUA list_windows.
2. Click its X.
3. Record that the window is immediately absent or invisible.
4. Read app_settings from the canonical database without modifying it and record the corresponding false flag.
5. Restart the fresh binary without CODEXBAR_PROOF_MODE.
6. Record that the closed surface does not return.
7. Re-enable it from Settings and record that it is recreated.
8. For FloatBall, confirm the prior collapsed position is restored.

Never include the account email, token, database row body, or local secret paths in committed evidence. Record only boolean flags, window labels/rectangles, DPI and pass/fail observations.

- [ ] **Step 10: Write the evidence record**

cua-observations.md starts with Glass Orbit Windows Verification and records these four values from commands rather than estimates:

- Binary timestamp and length from Get-Item target/debug/codex-barbar.exe.
- Commit SHA from git rev-parse HEAD.
- Windows scale from the CUA window state or GetDpiForWindow.
- CUA driver status as installed or unavailable.

Then add a Markdown table with exactly these rows and a final PASS/FAIL column: Taskbar ready layout, Float hover expansion, Taskbar X persists false, Float X persists false, Float position restore, Theme isolation. Every row cites the screenshot path plus its observed window rectangle or boolean state. If CUA is unavailable, label it unavailable and attach Win32 IsWindowVisible/GetWindowRect plus PrintWindow evidence as docs/WINDOWS_PROOF.md requires.

- [ ] **Step 11: Re-run final checks after evidence/doc edits**

~~~powershell
git diff --check
.\scripts\local-check.ps1 -Rust -Tauri -Frontend -Format -Clippy
git status --short
~~~

Expected: all checks PASS; status lists only Task 9 proof/docs files.

- [ ] **Step 12: Commit**

~~~powershell
git add apps/desktop-tauri/src-tauri/src/proof_harness.rs docs/WINDOWS_PROOF.md docs/verification/windows/2026-08-10
git commit -m "Verify glass orbit status surfaces"
~~~

---

## Final Acceptance Checklist

- [ ] Settings taskbar toggle, taskbar X and native taskbar close all use StatusSurfaceKind::TaskbarStatus.
- [ ] Settings float toggle, float X and native float close all use StatusSurfaceKind::FloatBall.
- [ ] No path persists false before runtime close reaches Destroyed or HiddenPendingDestroy.
- [ ] Persistence failure restores the prior visible/hidden runtime state.
- [ ] The 2-second monitor cleans disabled residual windows instead of returning early.
- [ ] Taskbar shows both 5-hour and weekly metrics, nearest reset, urgent value and an independent X.
- [ ] Float collapsed/expanded surfaces show the approved information hierarchy and independent X.
- [ ] Urgency uses usedPercent while visible numbers honor remaining/used display mode.
- [ ] Unknown usage windows never masquerade as five-hour or weekly.
- [ ] Missing data displays —/等待数据, not 0%.
- [ ] Cached stale quota remains visible and never replaces a signed-in identity with 未登录.
- [ ] Dragging never opens the panel and never overwrites saved collapsed geometry from an expanded move event.
- [ ] 100%, 150% and 200% pure geometry tests pass, including negative monitor coordinates.
- [ ] prefers-reduced-motion disables looping/positional animation.
- [ ] No new dependency or lockfile appears.
- [ ] Fresh Windows build, CUA/Win32 evidence, persistent close/restart and theme isolation are recorded.

## Execution Completion

After Task 9, invoke superpowers:verification-before-completion before claiming the feature is fixed. When all checks pass, invoke superpowers:requesting-code-review, then superpowers:finishing-a-development-branch to present merge/push choices. Do not push or open a pull request without explicit user authorization.
