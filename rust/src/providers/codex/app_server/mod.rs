//! Safe, supervised `codex app-server` stdio client internals.
//!
//! Everything under this module treats the Codex App Server as an
//! experimental, versioned dependency: executables are resolved fail-closed,
//! child processes are owned by a Windows Job Object, wire data is parsed
//! tolerantly, and product code only sees `AppError`, account identity, and
//! `ProfileUsageSnapshot`.

pub mod client;
pub mod codec;
pub mod discovery;
pub mod job;
pub mod model;
pub mod npm;
pub mod process;
pub mod protocol;
pub mod session;

pub use client::{
    AppServerNotification, ClientMetrics, CodexAppServerClient, INITIALIZE_TIMEOUT, REFRESH_BUDGET,
    RPC_TIMEOUT, SHUTDOWN_TIMEOUT,
};
pub use discovery::{
    CodexCommandResolver, CodexInstallation, ResolveRequest, ResolvedCodexCommand,
};
pub use model::{AccountIdentity, ParsedRateLimits, parse_profile_usage};
pub use process::{
    AppServerLaunchMode, AppServerProfileEnv, AppServerSpawnSpec, ChildEnvironment, FakeServerMode,
    SupervisedAppServerProcess,
};
pub use protocol::{InitializeCapabilities, InitializeClientInfo, InitializeParams};
pub use session::{
    AppServerFactory, CurrentCliSession, LocalAppServerFactory, LoginChallenge, LoginEvent,
    LoginFlow, ManagedSession,
};
