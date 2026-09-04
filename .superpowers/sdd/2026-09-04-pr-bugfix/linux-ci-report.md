# Linux CI compile repair report

## Scope

Fixed the Ubuntu-only compile and `-D warnings` failures reported by GitHub Actions run `33852751027`, job `100959027189`, on branch `codex/linux-deb`. No root-worktree files or remote PR state were modified.

## Root cause and repair

- `accounts/avatar.rs`: a Windows-only helper was imported by all test targets, and the Unix reader kept an unnecessary mutable `File` binding. The Windows test now qualifies the helper at its Windows-only call site, and the Unix binding is immutable.
- `accounts/vault/store.rs`: tests nested in `vault::store` resolved `super` to `store`, not the sibling `vault::crypto` and `vault::envelope` modules. The Linux tests now use fully qualified vault module paths. The non-Windows atomic-rename branch explicitly documents that `backup_path` belongs only to the Windows `ReplaceFileW` recovery path.
- `accounts/windows_acl.rs`: `RuntimeHomeManager` is imported only for the Windows-only DACL test.
- `providers/codex/app_server/npm.rs`: the Linux npm resolver now reads metadata from the validated `node` path before testing its Unix execute bits; `PathBuf` itself has no `permissions` method.
- `providers/codex/app_server/discovery.rs`: the Linux resolver explicitly ignores Windows-only `PATHEXT` while it probes bare `codex` commands.
- `providers/codex/app_server/process.rs`: the spawned child is mutable only after Windows-specific job setup needs mutation.

The existing Linux-only npm-layout test continues to cover the successful executable-node path; its prior failure was compilation of the resolver before the test could execute.

## Verification

Passed on this Windows host:

```text
cargo fmt --all
git diff --check
cargo test --manifest-path rust/Cargo.toml -- --test-threads=1
# 528 library tests passed; 17 contract tests passed; 3 icon tests passed; 1 ignored
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
cargo check --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml
cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml -- --test-threads=1
# 264 tests passed
cargo clippy --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml --all-targets -- -D warnings
```

`cargo check --manifest-path rust/Cargo.toml --target x86_64-unknown-linux-gnu` remains unavailable locally despite the target being installed: `ring v0.17.14` cannot find `x86_64-linux-gnu-gcc`. This is a host cross-compilation toolchain limitation before the crate is type-checked, not a replacement for the Ubuntu CI gate. The original Ubuntu log was read directly and supplied the target-specific diagnostics above.
