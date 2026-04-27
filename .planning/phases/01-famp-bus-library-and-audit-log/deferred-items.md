# Deferred Items

## 2026-04-27 — Out-of-scope clippy failure in dependency crate

- **Found during:** Plan 01-01 Task 2 verification hardening
- **Command:** `/Users/benlamm/.cargo/bin/cargo clippy -p famp-bus --lib -- -D warnings`
- **Issue:** Clippy checks path dependencies before `famp-bus`; `crates/famp-envelope/src/version.rs` fails `clippy::doc_markdown` on pre-existing documentation text for `audit_log MessageClass`.
- **Scope decision:** Out of scope for Plan 01-01, which is limited to `famp-bus` scaffold/primitives and the no-tokio gate. `cargo clippy -p famp-bus --lib --no-deps -- -D warnings` passes for the new crate itself.
