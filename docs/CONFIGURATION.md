<!-- generated-by: gsd-doc-writer -->
# FAMP Configuration Reference

This document covers every runtime configuration knob in the `famp` binary:
environment variables, CLI flags, config files, broker data-directory layout,
and Justfile recipes. The local-first v0.9 bus and the legacy v0.8 federation
paths each have distinct variables — the separation is noted where it matters.

---

## Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `FAMP_BUS_SOCKET` | No | `~/.famp/bus.sock` | Override the UDS broker socket path. Every CLI subcommand and the MCP server use this when set. Must be an absolute path. |
| `FAMP_LOCAL_IDENTITY` | No | — | D-01 identity resolution tier 2. Set to skip the cwd→`wires.tsv` lookup. Overridden by an explicit `--as` flag; overrides the wires.tsv match. Empty string is treated as unset. |
| `FAMP_LOCAL_ROOT` | No | `$HOME/.famp-local` | Override the MCP server's backing-store root (per-identity agent directories). Resolved by `famp mcp` only. |
| `FAMP_HOME` | No | `$HOME/.famp` | Override the identity home directory used by v0.8 federation commands (`init`, `info`, `config.toml`, `peers.toml`, keypair files). Must be an absolute path; relative paths are rejected with `HomeNotAbsolute`. |
| `HOME` | No (required if overrides absent) | OS default | Standard POSIX home directory. Used as the base when `FAMP_BUS_SOCKET`, `FAMP_LOCAL_ROOT`, and `FAMP_HOME` are all unset. `famp mcp` exits with `HomeNotSet` when `HOME` is absent and `FAMP_LOCAL_ROOT` is not set. |
| `FAMP_INSTALL_TARGET_HOME` | No | `dirs::home_dir()` | Hidden flag (also accepted as a CLI `--home` argument) that redirects `famp install-claude-code` and `famp daemon install` to a custom home directory. Intended for integration tests and CI; not for normal use. |
| `FAMP_RUN_LAUNCHCTL_TESTS` | No | — | When set (any value), enables launchctl integration tests in `tests/daemon_lifecycle.rs` and `tests/daemon_restart_binary_pickup.rs`. Test-only; not read by the binary at runtime. |

### Resolution precedence for the broker socket

```
$FAMP_BUS_SOCKET        (if set — taken verbatim)
$HOME/.famp/bus.sock    (default)
```

If `FAMP_BUS_SOCKET` is unset and `HOME` is unset, `resolve_sock_path` falls
back to `/nonexistent-famp-home/.famp/bus.sock` so the next syscall fails
visibly rather than silently writing into the current directory.

### Resolution precedence for the identity (D-01 four-tier chain)

```
1. --as <name>           CLI flag (Tier 1 — highest priority)
2. $FAMP_LOCAL_IDENTITY  Environment variable (Tier 2)
3. cwd → wires.tsv       cwd exact-match lookup in ~/.famp-local/wires.tsv (Tier 3)
4. Hard error            "no identity bound — pass --as after the subcommand …"
```

---

## Config File Format

### `config.toml` (v0.8 federation — under `FAMP_HOME`)

Location: `$FAMP_HOME/config.toml` (default: `~/.famp/config.toml`)

```toml
listen_addr = "127.0.0.1:8443"

# Optional: override the daemon's self-principal.
# When absent, the daemon uses agent:localhost/self.
# principal = "agent:localhost/alice"
```

| Field | Required | Default | Description |
|---|---|---|---|
| `listen_addr` | Yes | `"127.0.0.1:8443"` | TCP address for the v0.8 HTTPS federation listener (`famp listen`). |
| `principal` | No | `"agent:localhost/self"` | Self-principal used in envelope `from` fields. Useful when two daemons share a host. |

Unknown fields are rejected (`deny_unknown_fields`).

### `peers.toml` (v0.8 federation — under `FAMP_HOME`)

Location: `$FAMP_HOME/peers.toml` (default: `~/.famp/peers.toml`)

```toml
[[peers]]
alias = "bob"
endpoint = "https://bob.example.com:8443"
pubkey_b64 = "<base64url-unpadded ed25519 verifying key>"
# principal = "agent:bob.example.com/bob"   # optional
# tls_fingerprint_sha256 = "<sha256 hex>"   # set automatically on first contact
```

| Field | Required | Description |
|---|---|---|
| `alias` | Yes | Local nickname used in `famp send --to <alias>`. Must be unique. |
| `endpoint` | Yes | `https://host:port` of the remote daemon. |
| `pubkey_b64` | Yes | Base64url-unpadded Ed25519 verifying key (32 raw bytes when decoded). |
| `principal` | No | FAMP principal of the peer. Inferred as `agent:localhost/self` if absent. |
| `tls_fingerprint_sha256` | No | TOFU-pinned TLS certificate fingerprint. Written automatically on first successful contact. |

Unknown fields are rejected. An empty `peers.toml` (zero bytes) is valid and loads as an empty peer list.

### `~/.famp-local/wires.tsv` (v0.9 cwd-to-identity mapping — Tier 3)

Location: `$HOME/.famp-local/wires.tsv`

Tab-separated, one mapping per line:

```
/absolute/path/to/project-dir	alice
/absolute/path/to/other-dir	bob
```

Each row maps an absolute directory path to an identity name. Both the row's
directory and the process's cwd are canonicalized before comparison so symlinks
and trailing slashes do not cause misses. Lines with missing or empty fields are
skipped silently.

Managed by `famp-local wire <dir>` (the `famp-local` companion script, not the
`famp` binary). The `famp` binary reads it read-only during identity resolution.

---

## Required vs Optional Settings

### Settings that cause startup failure if absent

| Setting | Effect when absent |
|---|---|
| `HOME` (env) | `famp mcp` and any subcommand relying on `FAMP_HOME` default exit with `HomeNotSet` when neither `HOME` nor the relevant override is set. |
| `FAMP_HOME` value must be absolute | If set but relative, the binary exits immediately with `HomeNotAbsolute`. |
| `config.toml: listen_addr` | Required for v0.8 `famp listen`. Missing or malformed value causes a TOML parse error at startup. |

### Settings that are optional with safe defaults

All environment variables listed in the table above are optional. The binary
runs without any of them set, provided the OS `HOME` variable is present.

---

## Defaults

| Setting | Default value | Source |
|---|---|---|
| Broker socket path | `~/.famp/bus.sock` | `bus_client::resolve_sock_path` |
| MCP local root | `$HOME/.famp-local` | `cli::mcp::resolve_local_root` |
| v0.8 identity home | `$HOME/.famp` | `cli::home::resolve_famp_home` |
| `famp await` timeout | `30s` | `AwaitArgs::timeout` default |
| `famp wait-reply` timeout | `30s` | `WaitReplyArgs::timeout` default |
| v0.8 listen address | `127.0.0.1:8443` | `Config::default().listen_addr` |
| Broker idle exit | 300 s (5 min) | `IDLE_TIMEOUT` constant in `cli::broker` |
| `famp register` reconnect cap | 30 s | `RECONNECT_CAP` constant |

---

## Broker Data-Directory Layout

All broker runtime data lives under the directory that contains the socket
(`bus_dir = parent($FAMP_BUS_SOCKET)`, default `~/.famp/`):

```
~/.famp/
├── bus.sock                    # UDS broker socket (deleted on clean exit)
├── broker.log                  # stdout/stderr of the broker process
├── sessions.jsonl              # diagnostic-only session log (append-only)
└── mailboxes/
    ├── alice.jsonl             # agent mailbox (one envelope per line, canonical JSON)
    ├── #planning.jsonl         # channel mailbox
    ├── .alice.cursor           # per-identity read cursor (single ASCII decimal + newline)
    └── .#planning.cursor       # per-channel read cursor
```

The `mailboxes/` subdirectory is created automatically by the broker on first
start. Cursor files are managed client-side by `famp inbox ack --offset <N>`.

### v0.8 identity-home layout (under `FAMP_HOME`, default `~/.famp`)

```
~/.famp/                        # or $FAMP_HOME
├── config.toml                 # listen_addr, optional principal
├── peers.toml                  # federation peer registry
├── key.ed25519                 # Ed25519 signing key (private, mode 0600)
├── pub.ed25519                 # Ed25519 verifying key (public)
├── tls.cert.pem                # TLS certificate
├── tls.key.pem                 # TLS private key (mode 0600)
└── tasks/                      # per-task TOML records (famp-taskdir root)
```

---

## CLI Flags Reference

### Flags common to multiple subcommands

| Flag | Subcommands | Description |
|---|---|---|
| `--as <name>` | `send`, `await`, `wait-reply`, `inbox list`, `sessions`, `whoami`, `join`, `leave` | Override D-01 identity resolution (Tier 1). Takes precedence over `$FAMP_LOCAL_IDENTITY` and `wires.tsv`. |
| `--json` | `inspect broker`, `inspect identities`, `inspect tasks`, `inspect messages`, `inspect waiters` | Emit JSON output instead of a human-readable table. |

### `famp register <name>`

| Flag | Default | Description |
|---|---|---|
| `--tail` | off | Stream incoming envelopes to stderr at a 1-second poll cadence instead of blocking silently. |
| `--no-reconnect` | off | Exit non-zero on the first broker disconnect instead of reconnecting with exponential backoff. For tests and CI. |

### `famp broker`

| Flag | Default | Description |
|---|---|---|
| `--socket <path>` | `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock` | Override the UDS socket path for this broker instance. |
| `--no-idle-exit` | off | Disable the 300-second idle self-termination. Required for `famp daemon install` plist/unit invocations. |

### `famp send`

| Flag | Description |
|---|---|
| `--to <name>` | Direct-message recipient identity. Mutually exclusive with `--channel`. |
| `--channel <name>` | Channel target. Accepts `planning` or `#planning`; normalizes to `#planning`. Mutually exclusive with `--to`. |
| `--new-task <text>` | Open a new task with the given natural-language summary. Mutually exclusive with `--task`. |
| `--task <uuid>` | Continue an existing task (UUIDv7 from a prior `--new-task`). Mutually exclusive with `--new-task`. |
| `--terminal` | Mark the deliver envelope terminal — the final reply closing the task. Requires `--task`. |
| `--body <text>` | Optional freeform body text. |
| `--more-coming` | Signal that more briefing follows on a `--new-task` envelope. Requires `--new-task`. |

### `famp await`

| Flag | Default | Description |
|---|---|---|
| `--timeout <duration>` | `30s` | Block timeout. Accepts humantime durations: `30s`, `5m`, `250ms`, etc. |
| `--task <uuid>` | — | Optional task-id filter; broker returns only envelopes whose task matches. |

### `famp wait-reply`

| Flag | Default | Description |
|---|---|---|
| `--task <uuid>` | Required | Task id whose reply to wait for. Matched via `causality.ref`. |
| `--timeout <duration>` | `30s` | Block timeout after the inbox-first scan. |

### `famp inbox list`

| Flag | Default | Description |
|---|---|---|
| `--since <offset>` | `0` | Return only envelopes at or after this byte offset. |
| `--include-terminal` | off | Include envelopes for tasks already in a terminal FSM state. |

### `famp inbox ack`

| Flag | Description |
|---|---|
| `--offset <N>` | Advance the read cursor to this byte offset. No broker round-trip — purely a file write. |

### `famp sessions`

| Flag | Description |
|---|---|
| `--me` | Filter the session list to the caller's resolved identity only. |

### `famp inspect tasks`

| Flag | Description |
|---|---|
| `--id <uuid>` | Filter to a specific task id. |
| `--full` | Show each envelope in canonical JCS form. Requires `--id`. |
| `--orphans` | Show only tasks with no live holder. |

### `famp inspect messages`

| Flag | Description |
|---|---|
| `--to <name>` | Filter to messages addressed to this identity or channel. |
| `--tail <N>` | Limit to the N most-recent envelopes. |

### `famp daemon install` / `famp install-claude-code` / `famp install-codex`

| Flag | Env override | Description |
|---|---|---|
| `--home <path>` | `FAMP_INSTALL_TARGET_HOME` | Redirect all install writes to a custom home directory. Hidden; intended for integration tests. |
| `--project <path>` | `FAMP_INSTALL_CODEX_PROJECT_DIR` | For `install-codex` / `uninstall-codex`, choose the project root whose `.codex/hooks.json` receives or removes the FAMP Stop hook. Defaults to the current git root, or the current directory outside git. |

### `famp join <channel>`

| Flag | Description |
|---|---|
| `--role <role>` | Optional self-declared role (e.g. `"judge"`, `"peer"`) surfaced in the `JoinOk` member list. |

---

## Per-Environment Overrides

There are no separate `.env.development` / `.env.production` files. Environment
is controlled entirely through the variables above. The common patterns are:

### MCP server in Claude Code (production)

In `~/.claude.json` (written by `famp install-claude-code`):

```json
{
  "mcpServers": {
    "famp": {
      "command": "/Users/<you>/.cargo/bin/famp",
      "args": ["mcp"]
    }
  }
}
```

No `FAMP_HOME` or `FAMP_BUS_SOCKET` injection is needed — defaults resolve
from `HOME` at runtime.

### Two isolated identities on the same machine (e2e smoke test)

```bash
FAMP_BUS_SOCKET=/tmp/famp-smoke-a/bus.sock famp broker &
FAMP_BUS_SOCKET=/tmp/famp-smoke-b/bus.sock famp broker &
```

Each broker writes its mailboxes, cursor files, and logs into the parent
directory of its socket (`/tmp/famp-smoke-a/` and `/tmp/famp-smoke-b/`).

### CI / sandboxed shells

Broker auto-spawn is blocked by EPERM inside Claude Code and Codex sandboxes.
Use `famp daemon install` (from an unsandboxed shell) or start the broker
manually with `FAMP_BUS_SOCKET` pointing to a known path and keep it alive with
`--no-idle-exit`. Then set `FAMP_BUS_SOCKET` to that same path in the sandbox's
environment so all CLI calls reach the out-of-sandbox broker.

---

## Justfile Recipes

Run `just` with no arguments to list all available recipes.

| Recipe | Description |
|---|---|
| `just build` | Build the entire workspace with all targets. |
| `just test` | Run all tests via `cargo nextest`. |
| `just test-canonical` | Run `famp-canonical` tests only (fast feedback loop). |
| `just test-canonical-strict` | RFC 8785 conformance gate — no-fail-fast. Run on CI per PR. |
| `just test-canonical-full` | Run `famp-canonical` with the 100M float corpus. Nightly/release tags only (D-12). |
| `just test-crypto` | Run `famp-crypto` tests (RFC 8032 + §7.1c worked example). |
| `just test-core` | Run `famp-core` tests (wire-string fixtures + exhaustive-match gate). |
| `just test-doc` | Run workspace doc tests (`cargo nextest` does not run doctests). |
| `just lint` | Run `cargo clippy --workspace --all-targets -- -D warnings`. |
| `just fmt` | Format all sources with `cargo fmt --all`. |
| `just fmt-check` | Check formatting without modifying (CI gate). |
| `just install-hooks` | Install repo-local git hooks (`pre-commit`: fmt-check; `pre-push`: clippy). One-time per clone. |
| `just audit` | Run `cargo audit` for RustSec advisories. |
| `just install` | `cargo install --path crates/famp --locked --force` then `famp install-claude-code`. Run after any MCP tool surface change. |
| `just clean` | Remove build artifacts (`cargo clean`). |
| `just ci` | Full local CI-parity gate. Green here implies a green GitHub Actions run. |
| `just smoke-test` | Verify the quick-start install path (`cargo install --path crates/famp`) in an isolated root. |
| `just e2e-smoke` | Start two isolated broker daemons and print the `.mcp.json` snippets for a two-agent smoke test. |
| `just spec-lint` | Run the FAMP v0.5.1 spec anchor lint (ripgrep-based). |
| `just check-no-tokio-in-bus` | Assert `famp-bus` has no `tokio` in its dependency tree (BUS-01). |
| `just check-no-io-in-inspect-proto` | Assert `famp-inspect-proto` is I/O-free (INSP-CRATE-01). |
| `just check-inspect-readonly` | Assert `famp-inspect-server` imports no write surfaces (INSP-RPC-02). |
| `just check-inspect-version-aligned` | Assert inspector and broker share the same version of `famp-canonical`, `famp-envelope`, `famp-fsm` (INSP-CRATE-03). |
| `just check-mcp-deps` | Assert MCP/bus/broker source has no `use reqwest` or `use rustls` imports (MCP-01). |
| `just check-spec-version-coherence` | Prevent a split-commit between the `FAMP_SPEC_VERSION` bump and its implementation. |
| `just check-shellcheck` | Run `shellcheck` on `crates/famp/assets/hook-runner.sh` (D-08 invariant). |
| `just publish-workspace` | Publish all 12 workspace crates to crates.io in dependency order. Requires `cargo login` first. |
| `just publish-workspace-dry-run` | Dry-run all 12 crates to catch `Cargo.toml` publishability issues. |
| `just spike-tunnel` | Expose the local broker on a Tailscale tailnet via `socat` for cross-host testing. Zero FAMP code. |

---

## Daemon Service Files

### macOS (launchd)

Installed to `~/Library/LaunchAgents/com.famp.broker.plist` by
`famp daemon install`. Key properties:

| Property | Value |
|---|---|
| Label | `com.famp.broker` |
| ProgramArguments | `[<famp binary>, "broker", "--no-idle-exit"]` |
| RunAtLoad | `true` |
| KeepAlive | `true` |
| ProcessType | `"Background"` |
| StandardOutPath / StandardErrorPath | `~/.famp/broker.log` |

No `EnvironmentVariables` key is injected. The broker resolves `HOME` from the
launch context; all paths default normally.

### Linux (systemd --user)

Installed to `~/.config/systemd/user/famp-broker.service` by
`famp daemon install`. Requires systemd >= 240 for the `append:` log directive.
If linger is not enabled, the service stops on logout; enable persistence with:

```bash
loginctl enable-linger $USER
```

---

## Rust Toolchain

Pinned in `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.89.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

`rustup` resolves this automatically when you run any `cargo` command in the
workspace root.
