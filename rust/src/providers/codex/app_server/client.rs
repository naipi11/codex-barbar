//! Correlated, bounded stdio client for the Codex App Server.
//!
//! The client owns one supervised process, starts its stdout/stderr workers
//! before the initialize handshake, and never exposes raw protocol text or
//! child-process controls to callers. Requests are correlated by numeric IDs;
//! notifications are broadcast independently.

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex as StdMutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, broadcast, oneshot, watch};
use tokio::task::JoinHandle;

use crate::core::{AppError, AppErrorKind, RecoveryAction, SecretRedactor};

use super::codec::read_jsonl_message;
use super::process::{AppServerSpawnSpec, SHUTDOWN_GRACE, SupervisedAppServerProcess};
use super::protocol::{
    IncomingMessage, InitializeParams, InitializeResponse, RpcErrorBody, RpcId,
    encode_method_not_found, encode_notification, encode_request, parse_incoming_value,
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
const MAX_STDERR_LINE_BYTES: usize = 8 * 1024;

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

#[derive(Default)]
struct RequestState {
    pending: HashMap<RpcId, PendingSender>,
    terminal_error: Option<AppError>,
}

struct ClientInner {
    // Registration and terminal failure share one short, non-async lock. This
    // prevents EOF from draining the map just before a new request is inserted.
    requests: StdMutex<RequestState>,
    stdin: Mutex<Option<ChildStdin>>,
    terminal_changed: watch::Sender<Option<AppError>>,
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
            requests: StdMutex::new(RequestState::default()),
            stdin: Mutex::new(Some(stdin)),
            terminal_changed: watch::channel(None).0,
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
        let handshake = async {
            let initialize_params = serde_json::to_value(InitializeParams::v1())
                .map_err(|_| protocol_client_error("APP_SERVER_INITIALIZE_PARAMS_ENCODE_FAILED"))?;
            let response = client
                .request_with_timeout("initialize", initialize_params, INITIALIZE_TIMEOUT, true)
                .await?;
            InitializeResponse::validate(response)?;
            client.send_initialized().await
        }
        .await;
        if let Err(error) = handshake {
            client.shutdown().await?;
            return Err(error);
        }
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

    /// Wake notification consumers when the process exits, even while the
    /// session still owns a notification sender.
    pub(crate) async fn disconnected(&self) -> AppError {
        wait_for_terminal(&self.state.inner).await
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
        let sequence = self.state.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let id = RpcId::Number(
            i64::try_from(sequence)
                .map_err(|_| protocol_client_error("APP_SERVER_RPC_ID_EXHAUSTED"))?,
        );
        let frame = encode_request(id.clone(), method, params)?;
        let (sender, receiver) = oneshot::channel();
        let _pending = PendingRequest::register(&self.state.inner, id, sender)?;

        // Include writer lock acquisition and pipe backpressure in the budget,
        // not just the response wait. The guard also cleans up caller cancellation.
        let exchange = async {
            if let Err(error) = write_frame(&self.state.inner, &frame).await {
                set_terminal_error(&self.state.inner, error.clone());
                return Err(error);
            }
            receiver.await.unwrap_or_else(|_| {
                Err(terminal_error(&self.state.inner)
                    .unwrap_or_else(|| disconnected_error("APP_SERVER_RESPONSE_CHANNEL_CLOSED")))
            })
        };
        match tokio::time::timeout(timeout_duration, exchange).await {
            Ok(result) => result,
            Err(_) => {
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
        let frame = encode_notification("initialized", Value::Null)?;
        tokio::time::timeout(INITIALIZE_TIMEOUT, write_frame(&self.state.inner, &frame))
            .await
            .map_err(|_| timeout_error("APP_SERVER_INITIALIZE_TIMEOUT"))??;
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
        // Re-enter safely if an earlier shutdown future was cancelled.
        self.inner.closing.store(true, Ordering::Release);

        let shutdown_error = disconnected_error("APP_SERVER_CLIENT_SHUTDOWN");
        set_terminal_error(&self.inner, shutdown_error);
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

impl Drop for ClientState {
    fn drop(&mut self) {
        // Tokio detaches a task when its JoinHandle is dropped. Explicitly abort
        // stream workers on cancelled connect / last-client drop instead.
        self.inner.closing.store(true, Ordering::Release);
        set_terminal_error(&self.inner, disconnected_error("APP_SERVER_CLIENT_DROPPED"));
        if let Some(task) = self.reader_task.get_mut().take() {
            task.abort();
        }
        if let Some(task) = self.stderr_task.get_mut().take() {
            task.abort();
        }
        // Dropping the process closes its kill-on-close Job Object.
        drop(self.process.get_mut().take());
    }
}

struct PendingRequest<'a> {
    inner: &'a ClientInner,
    id: RpcId,
}

impl<'a> PendingRequest<'a> {
    fn register(
        inner: &'a ClientInner,
        id: RpcId,
        sender: PendingSender,
    ) -> Result<Self, AppError> {
        let mut requests = inner.requests.lock().expect("request state lock");
        if let Some(error) = &requests.terminal_error {
            return Err(error.clone());
        }
        requests.pending.insert(id.clone(), sender);
        Ok(Self { inner, id })
    }
}

impl Drop for PendingRequest<'_> {
    fn drop(&mut self) {
        self.inner
            .requests
            .lock()
            .expect("request state lock")
            .pending
            .remove(&self.id);
    }
}

struct FrameWriteGuard<'a> {
    inner: &'a ClientInner,
    complete: bool,
}

impl Drop for FrameWriteGuard<'_> {
    fn drop(&mut self) {
        if !self.complete {
            // A cancelled write_all may have sent a partial JSON line. Never
            // reuse that stream for another request.
            set_terminal_error(
                self.inner,
                disconnected_error("APP_SERVER_WRITE_INTERRUPTED"),
            );
        }
    }
}

async fn write_frame(inner: &Arc<ClientInner>, frame: &[u8]) -> Result<(), AppError> {
    tokio::select! {
        biased;
        error = wait_for_terminal(inner) => Err(error),
        result = async {
            let mut stdin = inner.stdin.lock().await;
            let stream = stdin.as_mut()
                .ok_or_else(|| disconnected_error("APP_SERVER_STDIN_CLOSED"))?;
            let mut guard = FrameWriteGuard { inner, complete: false };
            let result = async {
                stream.write_all(frame).await?;
                stream.flush().await
            }.await;
            if result.is_err() {
                set_terminal_error(inner, disconnected_error("APP_SERVER_WRITE_FAILED"));
            }
            guard.complete = true;
            result.map_err(|_| disconnected_error("APP_SERVER_WRITE_FAILED"))
        } => result,
    }
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
                    set_terminal_error(&inner, error);
                    break;
                }
            },
            Ok(None) => {
                if !inner.closing.load(Ordering::Acquire) {
                    inner.metrics.crashes.fetch_add(1, Ordering::Relaxed);
                }
                set_terminal_error(&inner, disconnected_error("APP_SERVER_EOF"));
                break;
            }
            Err(error) => {
                inner
                    .metrics
                    .protocol_errors
                    .fetch_add(1, Ordering::Relaxed);
                set_terminal_error(&inner, error);
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
        IncomingMessage::ServerRequest { id, .. } => {
            inner
                .metrics
                .protocol_errors
                .fetch_add(1, Ordering::Relaxed);
            let reply = async {
                let frame = encode_method_not_found(id)?;
                write_frame(inner, &frame).await
            };
            match tokio::time::timeout(RPC_TIMEOUT, reply).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => set_terminal_error(inner, error),
                Err(_) => set_terminal_error(inner, timeout_error("APP_SERVER_RPC_TIMEOUT")),
            }
        }
    }
}

async fn deliver_response(inner: &Arc<ClientInner>, id: RpcId, result: Result<Value, AppError>) {
    let sender = inner
        .requests
        .lock()
        .expect("request state lock")
        .pending
        .remove(&id);
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

async fn stderr_loop<R: AsyncRead + Unpin>(mut stderr: R) {
    let mut buffer = [0u8; 4096];
    let mut line = Vec::with_capacity(MAX_STDERR_LINE_BYTES);
    let mut discarded = false;
    let mut count = 0usize;
    loop {
        let read = match stderr.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        // The limit caps logging, not draining. Closing or blocking this pipe
        // after 1,000 lines can make the real server fail or hang.
        if count >= MAX_STDERR_LINES {
            continue;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' {
                if !discarded && count < MAX_STDERR_LINES {
                    log_stderr_line(&line);
                }
                count = count.saturating_add(1);
                line.clear();
                discarded = false;
            } else if count < MAX_STDERR_LINES && !discarded {
                if line.len() < MAX_STDERR_LINE_BYTES {
                    line.push(byte);
                } else {
                    // Never log a truncated credential: redaction may require
                    // its suffix. Discard oversized lines but keep draining.
                    line.clear();
                    discarded = true;
                }
            }
        }
    }
    if count < MAX_STDERR_LINES && !discarded && !line.is_empty() {
        log_stderr_line(&line);
    }
}

fn log_stderr_line(line: &[u8]) {
    let redacted = SecretRedactor::redact(&String::from_utf8_lossy(line));
    tracing::debug!(target: "codexbar::app_server", stderr = %redacted);
}

fn is_known_notification(method: &str) -> bool {
    matches!(
        method,
        "account/updated" | "account/rateLimits/updated" | "account/login/completed"
    )
}

fn terminal_error(inner: &ClientInner) -> Option<AppError> {
    inner
        .requests
        .lock()
        .expect("request state lock")
        .terminal_error
        .clone()
}

async fn wait_for_terminal(inner: &ClientInner) -> AppError {
    let mut receiver = inner.terminal_changed.subscribe();
    loop {
        if let Some(error) = receiver.borrow().clone() {
            return error;
        }
        if receiver.changed().await.is_err() {
            return disconnected_error("APP_SERVER_CLIENT_DROPPED");
        }
    }
}

fn set_terminal_error(inner: &ClientInner, error: AppError) {
    let senders = {
        let mut requests = inner.requests.lock().expect("request state lock");
        if requests.terminal_error.is_some() {
            return;
        }
        requests.terminal_error = Some(error.clone());
        requests
            .pending
            .drain()
            .map(|(_, sender)| sender)
            .collect::<Vec<_>>()
    };
    inner.terminal_changed.send_replace(Some(error.clone()));
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

    fn test_inner() -> Arc<ClientInner> {
        Arc::new(ClientInner {
            requests: StdMutex::new(RequestState::default()),
            stdin: Mutex::new(None),
            terminal_changed: watch::channel(None).0,
            notifications: broadcast::channel(NOTIFICATION_CAPACITY).0,
            metrics: AtomicMetrics::default(),
            next_id: AtomicU64::new(1),
            initialized: AtomicBool::new(true),
            closing: AtomicBool::new(false),
        })
    }

    fn test_client(inner: Arc<ClientInner>) -> CodexAppServerClient {
        CodexAppServerClient {
            state: Arc::new(ClientState {
                inner,
                process: Mutex::new(None),
                reader_task: Mutex::new(None),
                stderr_task: Mutex::new(None),
            }),
        }
    }

    #[tokio::test]
    async fn cancelled_pending_request_is_removed_and_terminal_state_blocks_registration() {
        let inner = test_inner();
        let (sender, receiver) = oneshot::channel();
        let pending = PendingRequest::register(&inner, RpcId::Number(1), sender).unwrap();
        drop(pending);
        assert!(inner.requests.lock().unwrap().pending.is_empty());
        assert!(receiver.await.is_err());
        let error = disconnected_error("APP_SERVER_EOF");
        set_terminal_error(&inner, error.clone());
        let (sender, _) = oneshot::channel();
        assert_eq!(
            PendingRequest::register(&inner, RpcId::Number(2), sender).err(),
            Some(error.clone())
        );
        assert_eq!(wait_for_terminal(&inner).await, error);
    }

    #[tokio::test]
    async fn string_response_id_never_steals_numeric_request() {
        let inner = test_inner();
        let (sender, receiver) = oneshot::channel();
        let _pending = PendingRequest::register(&inner, RpcId::Number(7), sender).unwrap();
        deliver_response(&inner, RpcId::String("7".to_string()), Ok(Value::Null)).await;
        assert_eq!(inner.requests.lock().unwrap().pending.len(), 1);
        deliver_response(&inner, RpcId::Number(7), Ok(serde_json::json!("numeric"))).await;
        assert_eq!(receiver.await.unwrap().unwrap(), "numeric");
        assert_eq!(inner.metrics.snapshot().unknown_responses, 1);
    }

    #[tokio::test]
    async fn rpc_budget_includes_writer_lock_and_cleans_up_pending_request() {
        let inner = test_inner();
        let client = test_client(Arc::clone(&inner));
        let _writer_lock = inner.stdin.lock().await;
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            client.request_with_timeout(
                "account/read",
                Value::Null,
                Duration::from_millis(25),
                false,
            ),
        )
        .await
        .expect("writer lock must not bypass RPC deadline");
        assert_eq!(
            result.unwrap_err().diagnostic_code,
            "APP_SERVER_RPC_TIMEOUT"
        );
        assert!(inner.requests.lock().unwrap().pending.is_empty());
        assert_eq!(inner.metrics.snapshot().rpc_timeouts, 1);
    }

    #[tokio::test]
    async fn last_client_drop_aborts_owned_workers() {
        let client = test_client(test_inner());
        let (sender, receiver) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let _sender = sender;
            std::future::pending::<()>().await;
        });
        *client.state.reader_task.lock().await = Some(task);
        drop(client);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), receiver)
                .await
                .expect("worker must be aborted, not detached")
                .is_err()
        );
    }

    #[tokio::test]
    async fn stderr_is_drained_after_logging_limit_and_oversized_lines() {
        let (mut writer, reader) = tokio::io::duplex(128);
        let drain = tokio::spawn(stderr_loop(reader));
        let write = async {
            writer
                .write_all(&vec![b'x'; MAX_STDERR_LINE_BYTES * 4])
                .await
                .unwrap();
            writer.write_all(b"\n").await.unwrap();
            for _ in 0..MAX_STDERR_LINES + 10 {
                writer.write_all(b"fixture line\n").await.unwrap();
            }
            writer
                .write_all(&vec![b'x'; MAX_STDERR_LINE_BYTES * 4])
                .await
                .unwrap();
            drop(writer);
            drain.await.unwrap();
        };
        tokio::time::timeout(Duration::from_secs(3), write)
            .await
            .expect("stderr must keep draining");
    }

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
