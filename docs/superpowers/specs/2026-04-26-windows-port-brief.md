# Windows Port — Agent Brief

**Status:** Research only, no code written. Hand this brief to an agent (or a future you) to execute.
**Date:** 2026-04-26
**Scope:** Port the FAMP Rust binary + library crates to Windows. Explicitly **does not** include porting the bash scaffolding scripts.
**Owner-on-pickup:** TBD

---

## TL;DR

The Unix surface is narrower than it looks. Pure protocol crates compile clean. The CLI has ~10 `cfg(unix)` sites that are mostly file-mode bits (`0o600`/`0o700`). The signal handler already has a Windows fallback. **The single real correctness risk is `crates/famp-inbox/src/lock.rs`, which has two latent Windows bugs hiding behind a "phase 3 doesn't target Windows" stub.**

Ship target: **Tier B** (CLI usable on Windows) in ~1 week. **Skip Tier D** (don't port `scripts/famp-local` to PowerShell — it's pre-v0.9 scaffolding and will be subsumed by the local-bus broker).

---

## Tiered execution plan

| Tier | Goal | Estimate |
|---|---|---|
| **A** | Library crates compile + protocol tests pass on Windows | 0.5–1 day |
| **B** | CLI usable on Windows: `init`, `send`, `listen`, `await` work end-to-end | 2–4 days |
| **C** | CI green on a Windows runner with full test suite | 3–5 days |
| **D** | Native Windows UX parity with `scripts/famp-local` | **DO NOT DO** — see "Out of scope" |
| **E** | v0.9 broker on Windows | Folds into v0.9 design; +2–3 days |

Default target for this work: **A → B → C, in that order.**

---

## What is and is not Unix-bound

### Pure (compiles on Windows today, no changes needed)

- `famp-canonical` — RFC 8785 JCS, no platform code
- `famp-crypto` — Ed25519, no platform code
- `famp-core` — protocol primitives
- `famp-fsm` — task FSM
- `famp-envelope` — envelope encoding
- `famp-transport` — trait crate
- `famp-transport-http` — axum/reqwest/rustls, all cross-platform

**Verification step for the agent:** `cargo build --target x86_64-pc-windows-msvc -p famp-canonical -p famp-crypto -p famp-core -p famp-fsm -p famp-envelope -p famp-transport -p famp-transport-http` from a Linux box with the windows target installed should succeed without modification.

### Unix-gated, needs Windows path

| File | What's gated | Difficulty |
|---|---|---|
| `crates/famp/src/cli/perms.rs` | `O_CREAT\|O_EXCL` + `mode(0o600)` for key files | Medium — see Bug Risk #3 |
| `crates/famp/src/cli/config.rs:144-147` | `set_permissions(0o600)` post-write | Easy — no-op or ACL |
| `crates/famp/src/cli/init/mod.rs:126-131` | `DirBuilderExt::mode(0o700)` for FAMP_HOME | Easy — no-op or ACL |
| `crates/famp/src/cli/listen/signal.rs` | SIGTERM handler, **already has Windows fallback at line 23** | Done — verify |
| `crates/famp-inbox/src/append.rs:40-64` | Mode 0600 on initial create, **already has `cfg(not(unix))` fallback** | Done — verify |
| `crates/famp-inbox/src/lock.rs` | PID liveness probe + Drop ordering | **HIGH — see Bug Risk #1, #2** |
| `crates/famp-inbox/src/cursor.rs:71-76` | Permissions on cursor file | Easy |
| `crates/famp-taskdir/src/store.rs:23-27` | Mode bits on task dir entries | Easy |
| `crates/famp-taskdir/src/atomic.rs:20-24` | Mode bits on atomic-write temp | Easy |

### Tests gated `#![cfg(unix)]`

~40 integration tests under `crates/famp/tests/`. Most gate only because they assert Unix mode bits or use `std::os::unix::fs::symlink`. Strategy:

1. Audit each gate in one pass — identify *why* it's gated.
2. Buckets:
   - **Mode-bit assertions** → split into a Unix-only assertion at the end + a cross-platform "behavior works" body.
   - **`symlink` usage** (`mcp_stdio_tool_calls.rs:138-139`) → use a small helper that maps to `std::os::windows::fs::symlink_dir`/`symlink_file` on Windows (requires SeCreateSymbolicLink privilege or Developer Mode — document this in CONTRIBUTING).
   - **Signal-killing tests** (`listen_shutdown.rs` — uses `libc::kill`) → gate-keep as Unix-only OR rewrite to use `Child::kill()` (cross-platform but coarser).
3. Anything that genuinely tests Unix-only semantics: keep `#![cfg(unix)]`. Don't force-port.

---

## HIGH-PRIORITY BUG RISKS

### Bug Risk #1: `is_alive` stub in `crates/famp-inbox/src/lock.rs:126-132` is wrong on Windows

```rust
#[cfg(not(unix))]
fn is_alive(_pid: u32) -> bool {
    // Phase 3 does not target Windows. Conservative: assume alive so we
    // never accidentally reap a live holder on a platform we have not
    // validated.
    true
}
```

**Symptom:** Any crashed `famp await` (or any holder) leaves a stale `inbox.lock` file. On next acquire, `is_alive(stale_pid)` returns `true`, and `InboxError::LockHeld` is raised forever. Recovery requires the user to manually delete `inbox.lock`. There is no programmatic recovery path.

**Fix:** Implement a real PID liveness check on Windows. Two options:

- **Option A (recommended): `sysinfo` crate.** ~5 lines. Avoids pulling `windows-sys`.
  ```rust
  #[cfg(windows)]
  fn is_alive(pid: u32) -> bool {
      use sysinfo::{Pid, System};
      let mut sys = System::new();
      sys.refresh_process(Pid::from_u32(pid));
      sys.process(Pid::from_u32(pid)).is_some()
  }
  ```
- **Option B: `windows-sys` direct.** `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)` + `GetExitCodeProcess`, check for `STILL_ACTIVE (259)`. Caveat: a process that legitimately exited with code 259 is misreported as alive. Rare but not zero.

Pick A unless there's a strong reason to avoid the dep.

### Bug Risk #2: `InboxLock::Drop` removes lock file while handle is still open (`lock.rs:100-107`)

```rust
impl Drop for InboxLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

`Drop::drop` runs **before** field destructors, so `_file` is still open when `remove_file` is called.

- **On Unix:** unlink-while-open is fine. Lock file disappears immediately.
- **On Windows:** `remove_file` returns `ERROR_SHARING_VIOLATION` because the handle was opened without `FILE_SHARE_DELETE`. The `let _ =` swallows the error → **lock file leaks on disk**.

Combined with Bug #1, this means **every `famp await` invocation on Windows will permanently brick the inbox lock for the next invocation.** First run leaves stale file; second run sees stale file, probes PID, gets "alive" (Bug #1), refuses.

**Fix (recommended):** restructure the struct so the file handle is dropped before the unlink.

```rust
pub struct InboxLock {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl Drop for InboxLock {
    fn drop(&mut self) {
        // Close the handle first, then unlink. Required on Windows.
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}
```

This is also subtly cleaner on Unix (deterministic close-before-unlink ordering), so apply unconditionally — don't `cfg(windows)` gate the fix.

### Bug Risk #3: `perms.rs` is structurally Unix-shaped

The whole module's purpose is "write a file at mode 0600 atomically." On Windows there are no POSIX modes; the analogous concept is an ACL granting only the current user.

**Two paths:**

- **Pragmatic (v1):** on Windows, write the file normally and document the security gap in `SECURITY.md`. Acceptable for a single-user developer environment; **unacceptable for federation production.** Add a startup warning when running on Windows: `WARN: key files are not ACL-protected on this platform; FAMP_HOME should be on a per-user-protected directory`.
- **Correct (v2):** use `windows-acl` or raw `windows-sys` SetSecurityInfo to apply an ACL granting only the current user. ~50 lines, but real Windows-API work and harder to test.

**Recommendation: ship v1 with the warning, file an issue for v2 before declaring Windows production-ready.**

### Bug Risk #4 (lower confidence — verify): atomic-write rename semantics

`crates/famp-taskdir/src/atomic.rs` uses a temp-file-then-rename pattern. On modern Rust (1.59+) `std::fs::rename` resolves to `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` on Windows, so atomic-replace works. **Verify** by reading the code and confirming it doesn't try to `rename` over an open handle (Windows will reject that even with `REPLACE_EXISTING`).

---

## Audit punch list (priority order)

Go through these in order. For each: read the file, identify any `cfg(unix)` site, decide fix or stub, write the patch.

1. **`crates/famp-inbox/src/lock.rs`** — Bug #1 + #2. Land first. Add property tests for crash-recovery (kill the process, restart, confirm reacquire works).
2. **`crates/famp-inbox/src/append.rs`** — Verify the `cfg(not(unix))` branch (already exists, lines 54-64). Confirm `tokio::fs::OpenOptions::append(true)` produces atomic appends on Windows (it should — Windows native append mode is atomic for writes ≤ pipe buffer; confirm for our envelope sizes).
3. **`crates/famp-inbox/src/cursor.rs`** — perms only, easy.
4. **`crates/famp-taskdir/src/atomic.rs`** — Bug #4 verification.
5. **`crates/famp-taskdir/src/store.rs`** — perms only.
6. **`crates/famp/src/cli/perms.rs`** — Bug #3. Pick pragmatic v1 path; add startup warning.
7. **`crates/famp/src/cli/config.rs`** — perms only.
8. **`crates/famp/src/cli/init/mod.rs`** — perms only on FAMP_HOME directory.
9. **`crates/famp/src/cli/listen/signal.rs`** — verify the Windows fallback (line 23) actually compiles + works. Spawn a process, send Ctrl-C, confirm graceful shutdown.
10. **TLS cert paths in `famp listen`** — confirm PEM loaders handle CRLF and Windows path separators. Likely fine via `rustls-pemfile`, but smoke-test once.

---

## Test strategy

1. **Local: cross-compile from macOS/Linux first.** `rustup target add x86_64-pc-windows-msvc` then `cargo check --target x86_64-pc-windows-msvc -p famp` catches 80% of issues without booting Windows. (Linker errors will block a full build; that's fine for a check.)
2. **Get to green on a Windows VM next.** GitHub Actions `windows-latest` runner is the cheapest VM. Run the full suite there.
3. **Un-gate tests one at a time.** Don't bulk-strip `#![cfg(unix)]`. Each gate exists for a reason; document the reason in the commit when removing.
4. **Add a CI matrix entry: `os: [ubuntu-latest, macos-latest, windows-latest]`.** Don't ship Windows support without it — the surface is too easy to silently break.
5. **Property tests for the lock module.** After fixing Bug #1 + #2, add `proptest` strategies that simulate: (a) holder crashes mid-execution, (b) two acquirers race, (c) PID reuse after holder death. These are the failure modes the Windows fallback was hiding.

---

## Out of scope (do NOT do these)

- **Do not port `scripts/famp-local` (748 lines bash) to PowerShell.** It's pre-v0.9 scaffolding for the local-bus broker design (see `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`). It will be subsumed by Rust subcommands when v0.9 lands. Effort spent here is throwaway.
- **Do not port `scripts/redeploy-listeners.sh` or `scripts/spec-lint.sh`.** Same reasoning — dev-loop scaffolding, not user-facing.
- **Do not refactor `unsafe_code = "forbid"` to allow Windows-API direct calls.** Use `windows-sys` (which respects the lint) or `sysinfo` (safe wrapper). The forbid is workspace-wide and load-bearing.
- **Do not chase v0.9 broker work in this task.** Tier E folds into the v0.9 build. This brief is Tier A → B → C only.
- **Do not declare Windows "production-ready" without solving Bug #3 properly.** v1 ships with a warning. Production parity requires real ACLs.

---

## Verification before marking complete

1. `cargo build --target x86_64-pc-windows-msvc --workspace` succeeds.
2. `cargo nextest run --workspace` passes on a `windows-latest` runner.
3. End-to-end smoke on Windows: `famp init`, `famp listen` in one window, `famp send` from another, `famp await` consumes. Do this **after** Bug #1 + #2 are fixed (otherwise `await` will brick on second run).
4. CI matrix entry for `windows-latest` is committed and green on a fresh PR.
5. `SECURITY.md` updated with the Windows ACL caveat (Bug Risk #3 v1 path).
6. Open a follow-up issue: "Windows: implement real ACL protection for key files (Bug Risk #3 v2)."

---

## References (in this repo)

- `crates/famp-inbox/src/lock.rs` — Bug #1, #2
- `crates/famp-inbox/src/append.rs:40-64` — existing Windows fallback to verify
- `crates/famp/src/cli/listen/signal.rs:14-26` — existing Windows fallback to verify
- `crates/famp/src/cli/perms.rs` — Bug #3
- `crates/famp/src/cli/init/mod.rs:126-131` — DirBuilder mode bits
- `docs/superpowers/specs/2026-04-17-local-first-bus-design.md` — v0.9 context (informs Tier E and "out of scope" reasoning)
- `Cargo.toml` workspace lints — `unsafe_code = "forbid"` is load-bearing
- `CLAUDE.md` "Architecture" section — federation vs local-bus split

---

## Open questions for the human before starting

1. Is Tier B (CLI usable, no CI matrix) acceptable, or is Tier C (full CI green on Windows) the bar?
2. Is the "v1 ship with warning, v2 real ACLs" plan for Bug Risk #3 acceptable, or does Windows need to ship with ACLs from day one?
3. Should the agent open a draft PR after Tier A and request review before continuing into Tier B/C?
