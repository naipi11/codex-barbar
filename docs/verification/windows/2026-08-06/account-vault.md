# Phase 2 account-vault verification — 2026-08-06

## Scope

This is a native Windows 11 x64 verification pass for the Phase 2 account
vault boundary. It does not claim that a real ChatGPT Managed login was
completed. No user credentials were copied, logged, or changed.

Environment: Windows 11 x64, build 26200. Codex CLI discovery reported
`codex-cli 0.146.0`.

## Evidence collected

| Check | Result | Evidence |
|---|---|---|
| Current User DPAPI round trip | PASS | `accounts::vault::crypto::tests::same_user_round_trip_and_wrong_profile_entropy` passed on the native Windows host. |
| DPAPI scope and UI flags | PASS | `accounts::vault::crypto::tests::dpapi_flags_are_current_user_and_ui_forbidden_only` passed; the test asserts `CRYPTPROTECT_LOCAL_MACHINE` is not present. |
| Wrong profile entropy does not decrypt | PASS | The same round-trip test returned `VaultError::UnprotectFailed` for a different Profile ID. This is same-user entropy isolation, not a second Windows-user test. |
| Protected runtime DACL shape | PASS | `accounts::windows_acl::tests::protected_directory_has_exact_dacl` passed; the implementation verifies a protected DACL with exactly two ACEs (Current User and SYSTEM). |
| Runtime/config isolation | PASS | `accounts::runtime_home::tests::managed_homes_are_distinct_and_force_file_auth` passed; each home is distinct and writes `cli_auth_credentials_store = "file"`. |
| Path traversal guards | PASS | `accounts::credential_bundle::tests::bundle_restore_rejects_parent_absolute_and_reparse_paths` passed for parent/absolute path inputs. An actual Windows reparse-point traversal test remains separate. |
| Crash/recovery invariants | PASS | Recovery and atomic-vault tests passed: newer runtime resealing preserves the previous valid Vault; invalid runtime data never overwrites it. |
| Current CLI immutability | PASS | `accounts::service::tests::current_cli_never_uses_managed_or_login_methods` passed. |
| Managed refresh lifecycle | PASS | `accounts::service::tests::managed_refresh_reseals_credentials_and_removes_runtime` passed with the fake App Server fixture. |
| Main CLI auth unchanged during this pass | PASS (bounded) | `%USERPROFILE%\.codex\auth.json` existed before and after the native test run. SHA-256 prefix/suffix remained `41239817…FE576`. No Managed login was attempted. |

Focused command:

```powershell
cargo test --manifest-path rust/Cargo.toml accounts:: -- --nocapture
```

Result: **27 passed, 0 failed** (898 filtered out).

## Not executed / not claimed

- A real ChatGPT browser/device-code Managed login was not started because it
  would require an interactive account action and a disposable account.
- Decryption under a different Windows user was not attempted; creating or
  switching Windows users is outside this verification pass.
- Force-kill at every live login/refresh checkpoint followed by relaunch was not
  performed against a real authenticated Managed session.
- A separately captured `icacls` output for an app-created runtime directory is
  not recorded; the exact-DACL assertion above is from the native Rust test.

These gaps remain Phase 2 exit-gate items. They are intentionally recorded as
unverified rather than inferred from unit tests.
