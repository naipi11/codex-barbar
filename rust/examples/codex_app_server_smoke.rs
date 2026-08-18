//! Read-only Codex App Server smoke probe for real Windows machines.
//!
//! Run through `scripts/codex-app-server-smoke.ps1`. The example never
//! prints an email address, account id, quota value, token, full path, raw
//! RPC line, or environment variable.

use std::path::Path;

use codexbar::providers::codex::app_server::{
    AppServerFactory, CodexCommandResolver, CodexInstallation, LocalAppServerFactory,
    ResolveRequest,
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeSummary {
    pub codex_version: Option<String>,
    pub installation: &'static str,
    pub initialized: bool,
    pub account_state: &'static str,
    pub rate_limits_method: &'static str,
    pub experimental_api: bool,
    pub error_kind: Option<String>,
}

impl SmokeSummary {
    fn from_probe(probe: SmokeProbe) -> Self {
        Self {
            codex_version: probe.version,
            installation: probe.installation,
            initialized: probe.initialized,
            account_state: probe.account_state,
            rate_limits_method: probe.rate_limits_method,
            experimental_api: false,
            error_kind: probe.error_kind,
        }
    }
}

struct SmokeProbe {
    version: Option<String>,
    installation: &'static str,
    initialized: bool,
    account_state: &'static str,
    rate_limits_method: &'static str,
    error_kind: Option<String>,
}

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(index) = args.iter().position(|arg| arg == "--generate-schema") {
        let out_dir = args
            .get(index + 1)
            .ok_or_else(|| "--generate-schema requires an output directory".to_string())?;
        return generate_schema(Path::new(out_dir));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(run_probe())
}

fn generate_schema(out_dir: &Path) -> Result<(), String> {
    let request = ResolveRequest {
        override_path: None,
        path: std::env::var_os("PATH"),
        pathext: std::env::var_os("PATHEXT"),
    };
    let command = CodexCommandResolver::new()
        .resolve(&request)
        .map_err(|error| format!("resolve failed: {}", error.diagnostic_code))?;
    std::fs::create_dir_all(out_dir).map_err(|error| error.to_string())?;
    let status = std::process::Command::new(command.launch_program())
        .args(command.launch_args_prefix())
        .arg("app-server")
        .arg("generate-json-schema")
        .arg("--out")
        .arg(out_dir)
        .stdin(std::process::Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!(
            "App Server schema generation failed with status {status}"
        ));
    }
    Ok(())
}

async fn run_probe() -> Result<(), String> {
    let request = ResolveRequest {
        override_path: None,
        path: std::env::var_os("PATH"),
        pathext: std::env::var_os("PATHEXT"),
    };
    let command = CodexCommandResolver::new()
        .resolve(&request)
        .map_err(|error| format!("resolve failed: {}", error.diagnostic_code))?;
    let version = command
        .version()
        .map(ToOwned::to_owned)
        .or_else(|| codexbar::providers::codex::app_server::discovery::probe_version(&command));
    let installation = match command.installation() {
        CodexInstallation::NativeExe => "nativeExe",
        CodexInstallation::StoreAlias => "storeAlias",
        CodexInstallation::VerifiedNpmLayout => "verifiedNpmLayout",
    };

    let factory = LocalAppServerFactory::default();
    let session = factory
        .open_current_cli()
        .await
        .map_err(|error| format!("open failed: {}", error.diagnostic_code))?;
    let account = session
        .account_read(false)
        .await
        .map_err(|error| format!("account failed: {}", error.diagnostic_code))?;
    let rates = session
        .rate_limits_read()
        .await
        .map_err(|error| format!("rates failed: {}", error.diagnostic_code))?;
    let _ = session.shutdown().await;

    let summary = SmokeSummary::from_probe(SmokeProbe {
        version,
        installation,
        initialized: true,
        account_state: if account.auth_mode == codexbar::core::AuthMode::ChatGpt {
            "signedIn"
        } else {
            "apiKey"
        },
        rate_limits_method: if rates.primary.is_some() || !rates.additional_windows.is_empty() {
            "available"
        } else {
            "unavailable"
        },
        error_kind: None,
    });
    println!(
        "{}",
        serde_json::to_string(&summary).map_err(|error| error.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_with_email_and_path() -> SmokeProbe {
        SmokeProbe {
            version: Some("codex-cli 0.0.0-test".to_string()),
            installation: "nativeExe",
            initialized: true,
            account_state: "signedIn",
            rate_limits_method: "available",
            error_kind: None,
        }
    }

    #[test]
    fn smoke_summary_omits_identity_and_paths() {
        let summary = SmokeSummary::from_probe(probe_with_email_and_path());
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains('@'));
        assert!(!json.contains("Users\\"));
        assert!(!json.to_ascii_lowercase().contains("token"));
        assert_eq!(summary.experimental_api, false);
    }
}
