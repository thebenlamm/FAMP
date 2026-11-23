# Migration: FAMP v1.0 -> v1.1

## TL;DR

- The local bus protocol version moved from `BUS_PROTO_VERSION` 1 to 2 (QUAR-10).
- Every `famp` binary must be **reinstalled** (`just install`, or `cargo install --path crates/famp` from a contributor checkout) and the daemon must be **restarted** (`famp daemon restart`).
- A v1.0 client talking to a v1.1 broker (or vice versa) **fails loudly by design**. This is not a bug and it is not silently degraded — it is the fail-closed security decision the quarantine boundary depends on.

## Why old clients are broken on purpose

v1.1 adds fail-closed provenance stamping to every mailbox record (Phase 14, the inbound-content-is-DATA quarantine — see [`docs/QUARANTINE.md`](QUARANTINE.md)). Every reply that can carry received content (`InboxOk`, `AwaitOk`, `RegisterOk`, `JoinOk`) now wraps each envelope in a `{"origin": ..., "envelope": ...}` record.

**An old client cannot render this provenance stamp.** Serving it to an old client anyway would mean deliberately handing unmarked remote content to a client that is blind to the mark — exactly the fail-open hole this phase exists to close. Graceful degradation here IS the vulnerability: a broker that tried to keep an old client working by silently omitting the stamp, or by shipping an "additive" reply shape, would be re-opening the exact gap QUAR-09's fail-closed design closes. (An additive sibling field was considered and rejected for a second, independent reason: `BusReply` itself is `#[serde(deny_unknown_fields)]`, so an old client would hard-fail decoding the new field anyway rather than gracefully ignoring it — there was no safe "purely additive" option on the table.)

Given that graceful degradation was never actually safe, the hard reject is deliberately the same mechanism this repo already uses for exactly this situation: `famp`'s existing version-skew handshake (VER-01) already hard-rejects on protocol mismatch and names the remedy in the error. Proto 2 uses the identical mechanism, not a new one.

## What breaks, and what does not

**Breaks until upgraded:**

- Every `famp` binary built before this bump — CLI and MCP paths both, since both go through the same `BusClient::connect` Hello handshake.
- Any foreign (non-`famp`) implementation of the local bus protocol, including a prior Grok interop session that has already wedged a mailbox once with a malformed envelope before this phase existed — expect a hard failure at Hello until it is updated to speak `bus_proto: 2` and understand the new reply shape.

**Explicitly NOT version-gated, and updated separately:** `famp_channel_log` reads its mailbox JSONL **directly from disk**, bypassing the broker and its Hello handshake entirely — the version check at Hello never runs for this path. It was updated in Phase 14 to parse the new `{"origin","envelope"}` on-disk record shape; its legacy branch (`famp_bus::split_stamped`'s fail-closed fallback) keeps it fail-closed against any record it cannot recognize as the new shape, including a v1.0-era bare-envelope line — those still resolve to `Origin::Unknown` and render marked, not local, not dropped.

**Unaffected:** every other CLI subcommand and MCP tool that only reads *counts* rather than envelope content (e.g. `famp inspect identities`, `famp_peers`) — these were not restructured and do not need special handling.

## On-disk consequence

Mailbox records written before v1.1 carry no stamp. Once you upgrade, reading a pre-v1.1 mailbox file resolves every one of those old lines to `Origin::Unknown`, and every rendering surface marks them accordingly. **This is correct, not a defect** — a v1.0-era record genuinely has unknown provenance under v1.1's fail-closed model, since v1.0 never recorded who sent it.

## Upgrade steps

```bash
# In your local FAMP clone
git pull
just install          # or: cargo install --path crates/famp --locked --force

# If you also run famp-gateway (cross-host federation):
just install-gateway

# Pick up the new broker binary:
famp daemon restart
```

Then restart any open Claude Code (or Codex) windows — they pick up the new binary on next launch.

If you skip either half of the upgrade (reinstall the client, or restart the daemon), the mismatch fails loudly rather than silently degrading: `BusClientError::ProtocolMismatch`'s message names both `just install` and `famp daemon restart` explicitly, matching the existing VER-01 error-message convention.

See also: the "Upgrading" section of [`README.md`](../README.md).
