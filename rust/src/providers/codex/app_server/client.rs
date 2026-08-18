//! Correlated, bounded stdio client for the Codex App Server.
//!
//! The client owns one supervised process, starts its stdout/stderr workers
//! before the initialize handshake, and never exposes raw protocol text or
//! child-process controls to callers. Requests are correlated by numeric IDs;
//! notifications are broadcast independently.

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio::task::JoinHandle;

use crate::core::{AppError, AppErrorKind, RecoveryAction, SecretRedactor};

use super::codec::read_jsonl_message;
use super::process::{AppServerSpawnSpec, SHUTDOWN_GRACE, SupervisedAppServerProcess};
use super::protocol::{
    IncomingMessage, InitializeParams, RpcErrorBody, RpcId, encode_notification, encode_request,
    parse_incoming_value,
};

/// Maximum time allowed for the initial capability handshake.
pub const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum time allowed for one post-initialize RPC.
pub const RPC_TIMEOUT: Duration = Duration::from_secs(20);
/// Budget for a complete account-plus-rate-limit refresh operation.
pub const REFRESH_BUDGET: Duration = Duration::from_secs(30);
/// Maximum graceful shutdown window before the process tree is killed.
pub const SHUTDOWN_TIMEOUT: Duration = SHUTDOWN_GRACE;

const NOTIFICATION_CAPACITY: usize = 64;
const MAX_STDERR_LINES: usize = 1_000;

/// A server notification delivered to subscribers.
#[derive(Debug, Clone, PartialEq)]
pub struct AppServerNotification {
    pub method: String,
    pub params: Value,
}

/// Stable, non-sensitive client metrics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientMetrics {
    pub initialized_notifications: u64,
    pub unknown_notifications: u64,
    pub protocol_errors: u64,
    pub unknown_responses: u64,
    pub rpc_timeouts: u64,
    pub initialize_timeouts: u64,
    pub crashes: u64,
}

#[derive(Default)]
struct AtomicMetrics {
    initialized_notifications: AtomicU64,
    unknown_notifications: AtomicU64,
    protocol_errors: AtomicU64,
    unknown_responses: AtomicU64,
    rpc_timeouts: AtomicU64,
    initialize_timeouts: AtomicU64,
    crashes: AtomicU64,
}

impl AtomicMetrics {
    fn snapshot(&self) -> ClientMetrics {
        ClientMetrics {
            initialized_notifications: self.initialized_notifications.load(Ordering::Relaxed),
            unknown_notifications: self.unknown_notifications.load(Ordering::Relaxed),
            protocol_errors: self.protocol_errors.load(Ordering::Relaxed),
            unknown_responses: self.unknown_responses.load(Ordering::Relaxed),
            rpc_timeouts: self.rpc_timeouts.load(Ordering::Relaxed),
            initialize_timeouts: self.initialize_timeouts.load(Ordering::Relaxed),
            crashes: self.crashes.load(Ordering::Relaxed),
        }
    }
}

type PendingSender = oneshot::Sender<Result<Value, AppError>>;

struct ClientInner {
    pending: Mutex<HashMap<RpcId, PendingSender>>,
    stdin: Mutex<Option<ChildStdin>>,
    terminal_error: Mutex<Option<AppError>>,
    notifications: broadcast::Sender<AppServerNotification>,
    metrics: AtomicMetrics,
    next_id: AtomicU64,
    initialized: AtomicBool,
    closing: AtomicBool,
}

struct ClientState {
    inner: Arc<ClientInner>,
    process: Mutex<Option<SupervisedAppServerProcess>>,
    reader_task: Mutex<Option<JoinHandle<()>>>,
    stderr_task: Mutex<Option<JoinHandle<()>>>,
}

/// A connected Codex App Server client.
#[derive(Clone)]
pub struct CodexAppServerClient {
    state: Arc<ClientState>,
}

impl CodexAppServerClient {
    /// Spawn a supervised process, start stream workers, and complete the
    /// `initialize`/`initialized` handshake.
    pub async fn connect(spec: AppServerSpawnSpec) -> Result<Self, AppError> {
        let mut process = SupervisedAppServerProcess::spawn(&spec)?;
        let stdin = match process.stdin() {
            Some(stdin) => stdin,
            None => {
                process.shutdown(SHUTDOWN_TIMEOUT).await;
                return Err(spawn_stream_error("APP_SERVER_STDIN_UNAVAILABLE"));
            }
        };
        let stdout = match process.stdout() {
            Some(stdout) => stdout,
            None => {
                process.shutdown(SHUTDOWN_TIMEOUT).await;
                return Err(spawn_stream_error("APP_SERVER_STDOUT_UNAVAILABLE"));
            }
        };
        let stderr = match process.stderr() {
            Some(stderr) => stderr,
            None => {
                process.shutdown(SHUTDOWN_TIMEOUT).await;
                return Err(spawn_stream_error("APP_SERVER_STDERR_UNAVAILABLE"));
            }
        };

        let (notification_sender, _) = broadcast::channel(NOTIFICATION_CAPACITY);
        let inner = Arc::new(ClientInner {
            pending: Mutex::new(HashMap::new()),
            stdin: Mutex::new(Some(stdin)),
            terminal_error: Mutex::new(None),
            notifications: notification_sender,
            metrics: AtomicMetrics::default(),
            next_id: AtomicU64::new(1),
            initialized: AtomicBool::new(false),
            closing: AtomicBool::new(false),
        });
        let state = Arc::new(ClientState {
            inner: Arc::clone(&inner),
            process: Mutex::new(Some(process)),
            reader_task: Mutex::new(None),
            stderr_task: Mutex::new(None),
        });
        {
            let mut task = state.reader_task.lock().await;
            *task = Some(tokio::spawn(reader_loop(Arc::clone(&inner), stdout)));
        }
        {
            let mut task = state.stderr_task.lock().await;
            *task = Some(tokio::spawn(stderr_loop(stderr)));
        }

        let client = Self { state };
        let initialize_params = serde_json::to_value(InitializeParams::v1())
            .map_err(|_| protocol_client_error("APP_SERVER_INITIALIZE_PARAMS_ENCODE_FAILED"))?;
        if let Err(error) = client
            .request_with_timeout("initialize", initialize_params, INITIALIZE_TIMEOUT, true)
            .await
        {
            client.shutdown().await?;
            return Err(error);
        }
        client.send_initialized().await?;
        inner.initialized.store(true, Ordering::Release);
        Ok(client)
    }

    /// Send one post-initialize request and await its correlated response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AppError> {
        if !self.state.inner.initialized.load(Ordering::Acquire) {
            return Err(protocol_client_error("APP_SERVER_NOT_INITIALIZED"));
        }
        self.request_with_timeout(method, params, RPC_TIMEOUT, false)
            .await
    }

    /// Subscribe to notifications without affecting request correlation.
    pub fn subscribe_notifications(&self) -> broadcast::Receiver<AppServerNotification> {
        self.state.inner.notifications.subscribe()
    }

    /// Snapshot counters accumulated by the reader/writer workers.
    pub fn metrics(&self) -> ClientMetrics {
        self.state.inner.metrics.snapshot()
    }

    /// Close stdin, allow a graceful exit for at most three seconds, then
    /// release the supervised process (which kills any surviving tree).
    pub async fn shutdown(self) -> Result<(), AppError> {
        self.state.shutdown().await;
        Ok(())
    }

    async fn request_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout_duration: Duration,
        initialize: bool,
    ) -> Result<Value, AppError> {
        if let Some(error) = terminal_error(&self.state.inner).await {
            return Err(error);
        }

        let id = RpcId(self.state.inner.next_id.fetch_add(1, Ordering::Relaxed));
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.state.inner.pending.lock().await;
            pending.insert(id, sender);
        }

        let frame = match encode_request(id, method, params) {
            Ok(frame) => frame,
            Err(error) => {
                self.state.inner.pending.lock().await.remove(&id);
                return Err(error);
            }
        };
        if let Err(error) = write_frame(&self.state.inner, &frame).await {
            self.state.inner.pending.lock().await.remove(&id);
            set_terminal_error(&self.state.inner, error.clone()).await;
            return Err(error);
        }

        match tokio::time::timeout(timeout_duration, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(terminal_error(&self.state.inner)
                .await
                .unwrap_or_else(|| disconnected_error("APP_SERVER_RESPONSE_CHANNEL_CLOSED"))),
            Err(_) => {
                self.state.inner.pending.lock().await.remove(&id);
                if initialize {
                    self.state
                        .inner
                        .metrics
                        .initialize_timeouts
                        .fetch_add(1, Ordering::Relaxed);
                    Err(timeout_error("APP_SERVER_INITIALIZE_TIMEOUT"))
                } else {
                    self.state
                        .inner
                        .metrics
                        .rpc_timeouts
                        .fetch_add(1, Ordering::Relaxed);
                    Err(timeout_error("APP_SERVER_RPC_TIMEOUT"))
                }
            }
        }
    }

    async fn send_initialized(&self) -> Result<(), AppError> {
        let frame = encode_notification("initialized", json!({}))?;
        write_frame(&self.state.inner, &frame).await?;
        self.state
            .inner
            .metrics
            .initialized_notifications
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

impl ClientState {
    async fn shutdown(&self) {
        if self.inner.closing.swap(true, Ordering::AcqRel) {
            return;
        }

        let shutdown_error = disconnected_error("APP_SERVER_CLIENT_SHUTDOWN");
        set_terminal_error(&self.inner, shutdown_error).await;
        {
            let mut stdin = self.inner.stdin.lock().await;
            *stdin = None;
        }

        if let Some(process) = self.process.lock().await.take() {
            // Keep the process wait bounded even if a fixture refuses to
            // close. Dropping the timed-out future releases the Job handle.
            let _ = tokio::time::timeout(
                SHUTDOWN_TIMEOUT + Duration::from_secs(1),
                process.shutdown(SHUTDOWN_TIMEOUT),
            )
            .await;
        }

        if let Some(task) = self.reader_task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(task) = self.stderr_task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

async fn write_frame(inner: &Arc<ClientInner>, frame: &[u8]) -> Result<(), AppError> {
    let mut stdin = inner.stdin.lock().await;
    let stream = stdin
        .as_mut()
        .ok_or_else(|| disconnected_error("APP_SERVER_STDIN_CLOSED"))?;
    stream
        .write_all(frame)
        .await
        .map_err(|_| disconnected_error("APP_SERVER_WRITE_FAILED"))?;
    stream
        .flush()
        .await
        .map_err(|_| disconnected_error("APP_SERVER_WRITE_FAILED"))?;
    Ok(())
}

async fn reader_loop(inner: Arc<ClientInner>, stdout: ChildStdout) {
    let mut reader = BufReader::new(stdout);
    loop {
        match read_jsonl_message(&mut reader).await {
            Ok(Some(value)) => match parse_incoming_value(value) {
                Ok(message) => handle_incoming(&inner, message).await,
                Err(error) => {
                    inner
                        .metrics
                        .protocol_errors
                        .fetch_add(1, Ordering::Relaxed);
                    set_terminal_error(&inner, error).await;
                    break;
                }
            },
            Ok(None) => {
                if !inner.closing.load(Ordering::Acquire) {
                    inner.metrics.crashes.fetch_add(1, Ordering::Relaxed);
                }
                set_terminal_error(&inner, disconnected_error("APP_SERVER_EOF")).await;
                break;
            }
            Err(error) => {
                inner
                    .metrics
                    .protocol_errors
                    .fetch_add(1, Ordering::Relaxed);
                set_terminal_error(&inner, error).await;
                break;
            }
        }
    }
}

async fn handle_incoming(inner: &Arc<ClientInner>, message: IncomingMessage) {
    match message {
        IncomingMessage::Response { id, result } => {
            deliver_response(inner, id, Ok(result)).await;
        }
        IncomingMessage::Error { id, error } => {
            deliver_response(inner, id, Err(map_rpc_error(error))).await;
        }
        IncomingMessage::Notification { method, params } => {
            if !is_known_notification(&method) {
                inner
                    .metrics
                    .unknown_notifications
                    .fetch_add(1, Ordering::Relaxed);
                return;
            }
            let _ = inner
                .notifications
                .send(AppServerNotification { method, params });
        }
        IncomingMessage::ServerRequest { .. } => {
            inner
                .metrics
                .protocol_errors
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

async fn deliver_response(inner: &Arc<ClientInner>, id: RpcId, result: Result<Value, AppError>) {
    let sender = inner.pending.lock().await.remove(&id);
    if let Some(sender) = sender {
        let _ = sender.send(result);
    } else {
        inner
            .metrics
            .unknown_responses
            .fetch_add(1, Ordering::Relaxed);
        inner
            .metrics
            .protocol_errors
            .fetch_add(1, Ordering::Relaxed);
    }
}

async fn stderr_loop(stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    let mut count = 0usize;
    while count < MAX_STDERR_LINES {
        match lines.next_line().await {
            Ok(Some(line)) => {
                count += 1;
                let redacted = SecretRedactor::redact(&line);
                tracing::debug!(target: "codexbar::app_server", stderr = %redacted);
            }
            Ok(None) | Err(_) => break,
        }
    }
}

fn is_known_notification(method: &str) -> bool {
    matches!(
        method,
        "initialized"
            | "account/updated"
            | "account/rateLimits/updated"
            | "account/login/completed"
            | "account/login/failed"
            | "account/login/cancelled"
            | "account/login/canceled"
    )
}

async fn terminal_error(inner: &Arc<ClientInner>) -> Option<AppError> {
    inner.terminal_error.lock().await.clone()
}

async fn set_terminal_error(inner: &Arc<ClientInner>, error: AppError) {
    let first = {
        let mut terminal = inner.terminal_error.lock().await;
        if terminal.is_none() {
            *terminal = Some(error.clone());
            true
        } else {
            false
        }
    };
    if !first {
        return;
    }

    let senders = {
        let mut pending = inner.pending.lock().await;
        pending
            .drain()
            .map(|(_, sender)| sender)
            .collect::<Vec<_>>()
    };
    for sender in senders {
        let _ = sender.send(Err(error.clone()));
    }
}

fn map_rpc_error(error: RpcErrorBody) -> AppError {
    if error.code == -32601 {
        return AppError::new(
            AppErrorKind::UnsupportedCodexVersion,
            "errors.appServerMethodUnavailable",
            RecoveryAction::InstallTestedCodex,
            "APP_SERVER_METHOD_NOT_FOUND",
        );
    }
    AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.appServerRpcError",
        RecoveryAction::Retry,
        "APP_SERVER_RPC_ERROR",
    )
}

fn spawn_stream_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::CodexNotFound,
        "errors.appServerSpawnFailed",
        RecoveryAction::InstallTestedCodex,
        code,
    )
}

fn disconnected_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::OfflineOrTimeout,
        "errors.offlineOrTimeout",
        RecoveryAction::Retry,
        code,
    )
}

fn timeout_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::OfflineOrTimeout,
        "errors.offlineOrTimeout",
        RecoveryAction::Retry,
        code,
    )
}

fn protocol_client_error(code: &'static str) -> AppError {
    AppError::new(
        AppErrorKind::ProtocolMismatch,
        "errors.appServerProtocolMismatch",
        RecoveryAction::InstallTestedCodex,
        code,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_keep_the_phase_one_budgets() {
        assert_eq!(INITIALIZE_TIMEOUT, Duration::from_secs(10));
        assert_eq!(RPC_TIMEOUT, Duration::from_secs(20));
        assert_eq!(REFRESH_BUDGET, Duration::from_secs(30));
        assert_eq!(SHUTDOWN_TIMEOUT, Duration::from_secs(3));
    }

    #[test]
    fn metrics_are_stable_and_non_sensitive() {
        let metrics = ClientMetrics::default();
        let json = serde_json::to_string(&serde_json::json!({
            "initializedNotifications": metrics.initialized_notifications,
            "unknownNotifications": metrics.unknown_notifications,
            "protocolErrors": metrics.protocol_errors,
        }))
        .unwrap();
        assert!(!json.contains("token"));
        assert!(json.contains("unknownNotifications"));
    }
}
