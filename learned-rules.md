# Learned Rules — FAMP

Rules extracted from real failures during FAMP development. Each rule cost
real debug time; all are worth applying automatically in future sessions
without rediscovery. Consumed at session start via project CLAUDE.md.

---

<!-- Rules will be appended below this line -->

### Rule: daemon-keyring-cache-ordering
**Learned**: 2026-04-17
**Trigger**: `famp-local` wrapper added a new peer to `peers.toml` *after* the listen daemons had already started. The receiver's in-memory keyring still reflected the pre-mutation peer list, so signed envelopes from the new peer were rejected with `HTTP 401 Unauthorized` from the daemon — with no indication in the daemon log that the rejection had happened.
**Correction**: Populate `peers.toml` **before** running `famp listen`. When adding a new peer to an existing mesh, stop all running daemons first, edit the config, then `init` again so every daemon loads the new keyring at fresh startup.
**Rule**: The v0.8 `famp listen` daemon loads its peer keyring **once at startup** and does not re-read `peers.toml` afterward. Any code path that mutates a daemon's `peers.toml` (adding a pubkey, pinning a fingerprint, removing a peer) MUST restart that daemon, otherwise inbound envelopes from the affected peer will 401 with no useful log signal.
**Scope**: project-famp

### Rule: peer-import-uses-stderr-for-both-success-and-error
**Learned**: 2026-04-17
**Trigger**: `famp-local wrap`'s `pair_one_way` helper captured `peer import`'s stderr and matched it against `"peer already exists"` to swallow idempotent failures. That worked for the error case but broke the success case — `peer import` writes *"Peer imported successfully"* also to **stderr**, not stdout. String-matching treated success as an unknown error and aborted init under `set -euo pipefail`.
**Correction**: Key idempotency on the **exit code**, not stderr content. `rc == 0` → success (discard any informational stderr); `rc != 0` AND stderr matches `"peer already exists"` → swallow; anything else → propagate.
**Rule**: `famp peer import` writes status messages (both success and failure) to stderr. Never shell-parse its stderr to detect outcome. Always dispatch on `$?` first and only inspect stderr to disambiguate non-zero cases.
**Scope**: project-famp

### Rule: apple-398-day-serverauth-cert-limit
**Learned**: 2026-04-17
**Trigger**: v0.8's `famp init` generated self-signed TLS certs with a 10-year validity. All 5 `famp listen` integration tests failed on macOS with `InvalidCertificate(Other(OtherError("famp-local certificate is not standards compliant: -67901")))`. The devbox (Linux, webpki path) never enforced this policy, so the regression shipped silently.
**Correction**: Issue TLS serverAuth certs with ≤397-day validity. Also explicitly set `KeyUsage::DigitalSignature` and `ExtendedKeyUsage::ServerAuth` — Apple's platform verifier rejects certs missing either bit, regardless of validity period.
**Rule**: macOS `Security.framework` (used by `rustls-platform-verifier` on Darwin) enforces the CA/B Forum baseline: serverAuth certificates with `(notAfter - notBefore) > 398 days` fail validation with `errSecCertificateNotStandardsCompliant` (-67901). Any code generating self-signed certs for FAMP must clamp validity to 397 days and include `DigitalSignature` KU + `ServerAuth` EKU, otherwise every Mac user fails `just ci` out of the box. Linux CI does not catch this — add a macOS matrix runner or verify manually.
**Scope**: project-famp

### Rule: famp-cli-error-source-chain-not-rendered-by-default
**Learned**: 2026-04-17
**Trigger**: `famp send` failures printed literal `"send failed"` with no detail. The underlying `reqwest::Error` (e.g. `HTTP 401`, TLS handshake failure, DNS error) was attached as `#[source]` on `CliError::SendFailed` via thiserror but never rendered, because `main.rs` used `eprintln!("{e}")` which only invokes `Display` on the top-level variant. Debugging took ~20 minutes before patching main.rs to print the source chain revealed the actual cause.
**Correction**: Walk `std::error::Error::source()` in main.rs and print each cause on its own line as `caused by: …`. Fixed in commit `7f9a7b0`.
**Rule**: When diagnosing a `famp` CLI failure that prints only a terse top-level message ("send failed", "envelope encode/sign failed"), the underlying error chain is almost always the useful part — since commit `7f9a7b0` (2026-04-17), `main()` walks and prints that chain, but for future `thiserror` variants the `#[source]` attribute is mandatory. Never write an error variant that swallows the underlying cause without attaching it via `#[source]`.
**Scope**: project-famp

### Rule: daemons-cache-keyring-use-bind-exclusion-not-pid-file
**Learned**: 2026-04-17 (applied in v0.9 design spec)
**Trigger**: Original v0.9 broker-lifecycle design used double-fork + PID file + `flock` + reference counting for single-instance enforcement. `zed-velocity-engineer` review flagged this as a multi-week edge-case surface (macOS `SIGHUP` on Terminal.app Cmd-Q, `launchd`'s session reaper, PID reuse after reboot, fd inheritance across fork).
**Correction**: Use `bind()` on the Unix domain socket as the exclusion primitive. `EADDRINUSE` → another broker is running (connect-probe to distinguish live vs. stale); `ECONNREFUSED` on probe → stale socket, `unlink()` and retry bind. No PID file, no flock, no ref-counting. The socket IS the lock.
**Rule**: For same-host, single-instance daemons with a known socket path, prefer UDS-bind-as-exclusion over PID files + flock. Less platform-specific edge-case surface, no zombie/reuse races, no cleanup ceremony. PID files are for process-discovery (give me the pid so I can SIGTERM it), not for mutual exclusion.
**Scope**: project-famp

### Rule: federation-code-preservation-requires-running-ci
**Learned**: 2026-04-17 (the-architect plumb-line)
**Trigger**: v0.9 spec originally preserved `famp-transport-http` + `famp-keyring` crates as "v1.0 federation internals" with no required CI coverage — the crates would sit in the workspace, unused by top-level CLI, quietly bit-rotting. `the-architect` review called this *mummification* disguised as preservation.
**Correction**: Phase 4 now has a **hard requirement**: `e2e_two_daemons` is refactored to target `famp-transport-http`'s library API (not the soon-to-be-deleted CLI subcommands). Test stays green in CI on every commit. If the test dies, the preservation thesis dies with it — either the crates are honestly archived (delete + git tag) or the CI commitment is restored.
**Rule**: Keeping code "for later" in a live workspace only counts as preservation if that code is exercised in CI on every commit. Otherwise the code is a tomb and git history is the real preservation mechanism. For any v0.9 / v1.0 work that preserves federation primitives, verify federation E2E tests still run against them before claiming the preservation is real.
**Scope**: project-famp

### Rule: stable-mcp-tool-surface-across-protocol-versions
**Learned**: 2026-04-17 (applied in v0.9 design spec)
**Trigger**: Considering how users adopt FAMP across v0.8 → v0.9 → v1.0, it was tempting to let each version have its own MCP tool names (`famp_send_tls`, `famp_send_bus`, `famp_send_federated`, etc).
**Correction**: The MCP tool surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) means the *same semantic thing* in v0.9 (bus), v1.0 (bus + gateway routing), and every future version. What changes under the hood is the router; users (and Claude Code integrations) never retrain vocabulary.
**Rule**: The MCP tool surface is a cross-version user contract. When designing new FAMP features, adding a tool that replaces an existing one is a breaking change for every Claude Code user. Extend existing tools (new optional args, richer return types, additional variants on enum fields), don't rename or replace. the-architect called this "the single most valuable decision in the whole design."
**Scope**: project-famp
