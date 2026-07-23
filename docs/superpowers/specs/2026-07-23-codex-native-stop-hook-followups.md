# Codex Native Stop Hook — Follow-Up Items

**Written:** 2026-07-23 · **For:** whoever picks this up in a fresh window
**Read [`2026-07-23-codex-native-stop-hook-design.md`](2026-07-23-codex-native-stop-hook-design.md) first.** This supplements it.

These are the items an adversarial review of the native Codex Stop-hook
changeset raised that were **deliberately not fixed** in that changeset. Every
blocker and functional bug found in the same review *was* fixed and shipped.
Nothing here is a known-broken wake path; these are hardening, hermeticity, and
documentation debt.

---

## 0. State you are inheriting

Shipped in the accompanying commit:

- `famp hook codex-stop` — native Rust Stop-hook lifecycle (`crates/famp/src/cli/hook/`).
- `install-codex` / `uninstall-codex` wire and remove the native command
  (`<abs famp> hook codex-stop`), prune the legacy `famp-await.sh` shim, and
  handle both native and legacy trust hashes.
- A pre-write support probe (`famp hook codex-stop --help`) so `install-codex`
  refuses to wire a `famp` binary that predates the subcommand.
- `FAMP_INSTALL_FAMP_BIN` to pin the wired binary (see `docs/CONFIGURATION.md`).

Verified at commit time: `cargo clippy --workspace --all-targets` = 0 errors;
`cargo test -p famp` green except the two pre-existing
`famp_local_wire_migration` failures (reproduced identically on `ca7cbee` in a
clean worktree — legacy famp-local daemon port bind, unrelated to this work).

**Verified premise:** Codex 0.144.6 executes hook `command` strings through
`$SHELL -lc` (`codex_hooks::engine::command_runner` + `SHELL` / `-lc` in the
shipped binary). A multi-token command string is therefore valid. Re-verify if
Codex ever changes hook dispatch.

**Operational note:** `~/.cargo/bin/famp` must be reinstalled (`just install`)
before running `famp install-codex`, since the hook command points at the
deployed binary, not the build tree.

---

## 1. The deleted `-amended` spec took the parity matrix with it

**Severity: highest of the items here — this is the one that loses information.**

`2026-07-23-codex-native-stop-hook-design-amended.md` (529 lines) was deleted
when its content was folded into the main design doc. The fold kept the prose
but dropped two normative artifacts:

- the **listen-state decision table** (event → effect on `active` / `last_identity`)
- the **parity matrix P01–P25** (shell → native), which carried the rule
  *"Any **Keep** row omitted from implementation is a regression."*

That matrix was the acceptance checklist for this work, and the rows it lists
are precisely where the implementation still diverges: **P03** (stdin
disconnect), **P14** (queue-watch / `--abort-on-fd`, now documented as deferred),
**P16**, **P18**. Recover it from git history and fold the table + matrix into
the surviving design doc:

```
git show ca7cbee:docs/superpowers/specs/2026-07-23-codex-native-stop-hook-design-amended.md
```

---

## 2. `emit.rs` unit test reaches the real user broker

`crates/famp/src/cli/hook/emit.rs:209` — `emits_native_json_without_jq` calls
`emit_block_decision`, which for agent mailboxes calls `actionable_unread` →
`resolve_sock_path()` (line 137) with no socket override. It talks to whatever
broker the developer has running.

It passes today only because no identity named `dk` is registered. A live `dk`
session with `mailbox_unread == 0` makes the `#26` suppression path fire and the
test goes red for reasons unrelated to the code under test.

Fix: give `emit_block_decision` an injectable socket path (there is already an
`actionable_unread_at(sock, identity)` seam directly beneath it), and have the
test point at a temp path with no listener so the inspect probe fails open.

---

## 3. `walk_sessions` follows symlinks with no depth cap

`crates/famp/src/cli/hook/codex_rollout.rs:125-131` — recursion is gated on
`path.is_dir()`, which follows symlinks. A symlink cycle anywhere under
`$CODEX_HOME/sessions` hangs or stack-overflows the hook, on the critical path,
per Stop.

Fix: cap recursion depth (rollouts live at `sessions/YYYY/MM/DD/`, so 4–6 is
generous) and/or skip symlinked directories via `symlink_metadata`.

---

## 4. SQLite URI is built without escaping

`crates/famp/src/cli/hook/codex_rollout.rs:83` —
`format!("file:{}?mode=ro", db.display())`. A `CODEX_HOME` containing `?`, `#`,
or a space produces a malformed URI. It fails open to the glob path, so the
consequence is silent degradation rather than breakage — but it degrades without
a log line saying so.

Fix: percent-encode the path, or open by `Path` with
`SQLITE_OPEN_READ_ONLY` and drop the URI form entirely.

---

## 5. `mcp_tool_call_end` ignores `invocation.server`

`crates/famp/src/cli/hook/transcript.rs:241-247` — the `function_call` branch
checks `payload.namespace` against `mcp__famp` (line 208), but the
`mcp_tool_call_end` branch matches on `tool.ends_with("famp_register")` alone
and never reads `invocation.server`, even though the field is present in every
real event (and in the fixtures at lines 406 / 414).

Any MCP server exposing a tool whose name ends in `famp_register` /
`famp_set_listen` can therefore inject a listen identity. Low practical risk,
but it is an anti-hijack asymmetry against a rule the sibling branch enforces.

Fix: require `invocation.server == "famp"` (or empty) in the
`mcp_tool_call_end` branch. Add a fixture with a foreign server name asserting
`ListenState::Unresolved`.

---

## 6. P03: stdin is not actually disconnected

`crates/famp/src/cli/hook/codex_stop.rs:45` carries the comment
`// Disconnect-equivalent: we already consumed stdin fully.` The shell adapter
did a real `exec 0</dev/null` before its long await, specifically to avoid
holding the host's pipe across a 23h park.

Reading to EOF is not equivalent — the fd stays open. Practical impact is low
(Rust ignores `SIGPIPE` at startup, and the risk is on writes), so this is
recorded as an honesty fix: either close fd 0 after the read, or replace the
comment with what actually happens and why it is acceptable.

---

## 7. Hook log: format change and unbounded growth

`crates/famp/src/cli/hook/log.rs:29-35` emits raw epoch seconds. The shell
adapter emitted `date -Iseconds`. Anything grepping
`$XDG_STATE_HOME/famp/await-hook.log` by date breaks, and the two formats are
now interleaved in the same file during migration.

Separately, the file is opened append-only with no rotation (line 47), on a path
that writes several lines per turn per identity indefinitely.

Fix: emit RFC3339 (`humantime::format_rfc3339_seconds` is already in the tree,
no new dependency), and add a size check with a single `.1` rollover.

---

## 8. Known-red, not ours

`crates/famp/tests/famp_local_wire_migration.rs` — `wire_rewrites_legacy_mcp_json_in_place`
and `wire_idempotent_on_already_migrated_file` fail deterministically with
`famp-local: alice: daemon failed to bind port <N> within 1s`. Confirmed
pre-existing on `ca7cbee`. Legacy v0.8 famp-local path; untouched by this work.
Do not let it mask a real regression — baseline before assuming.

---

## Suggested order

1. **§1** — restore the parity matrix before anything else; it is the checklist
   the remaining items are measured against, and it is only recoverable from
   git history.
2. **§3**, **§5** — the two that can actually bite a user (hook hang; identity
   injection).
3. **§2** — unblocks trustworthy CI on the hook module.
4. **§4**, **§6**, **§7** — cleanup.
