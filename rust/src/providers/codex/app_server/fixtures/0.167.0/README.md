# Codex 0.167.0 account and quota fixtures

These deterministic, synthetic response bodies exercise the official `rust-v0.167.0` contract. They are not captures of a signed-in account and contain no credentials. Existing 0.146.0 fixtures remain unchanged.

## Pinned upstream sources

- [Release 0.167.0](https://github.com/openai/codex/releases/tag/rust-v0.167.0), published 2026-11-06T01:40:00Z.
- Annotated tag `cb41599ccbe371272892f156286a236aa097bada`, commit `086ccc833bc9cf01024ad2dd6af7dbd421b6bf9a`.
- [GetAccountResponse schema](https://github.com/openai/codex/blob/086ccc833bc9cf01024ad2dd6af7dbd421b6bf9a/codex-rs/app-server-protocol/schema/json/v2/GetAccountResponse.json).
- [GetAccountRateLimitsResponse schema](https://github.com/openai/codex/blob/086ccc833bc9cf01024ad2dd6af7dbd421b6bf9a/codex-rs/app-server-protocol/schema/json/v2/GetAccountRateLimitsResponse.json).
- [Account processor](https://github.com/openai/codex/blob/086ccc833bc9cf01024ad2dd6af7dbd421b6bf9a/codex-rs/app-server/src/request_processors/account_processor.rs#L1171-L1287): read returns the complete bucket map; the legacy view prefers `codex` and otherwise uses the first backend snapshot.

## Coverage

- [account-chatgpt.json](account-chatgpt.json): nullable email, an official usage-based plan, and workspace display name. Workspace metadata must not become a fabricated personal display name. Inline tests also cover API-key, signed-out, unsupported cloud account variants, and malformed account envelopes.
- [rate-limits-multi-bucket.json](rate-limits-multi-bucket.json): legacy and named Codex views, a separate quota bucket, balance credits, spend controls, root account/permission/banner metadata, and reset-credit details. Monetary `individualLimit.used` is not a window percentage. `availableCount` is authoritative even when credit details are truncated.
- [rate-limits-nullable.json](rate-limits-nullable.json): every relevant optional field set to `null`, including a valid window with unknown duration/reset time.

Tests derive malformed variants from these bodies to check bucket isolation and fallbacks. Unsupported or missing information is never synthesized from credit balances, spend-control amounts, workspace metadata, or arbitrary buckets. The full-read parser does not implement sparse notification merging.
