# Security Policy

FAMP is a reference implementation of a federated agent messaging protocol.
Its core value proposition is **byte-exact, signature-verifiable interop**.
Vulnerabilities in canonicalization, signing, signature verification, or
trust boundary code are taken seriously and handled privately.

## Reporting a Vulnerability

**Do not open public GitHub issues for security vulnerabilities.**

Please report via one of the following channels:

1. **GitHub private security advisory** (preferred) —
   https://github.com/thebenlamm/FAMP/security/advisories/new
2. **Email** — `benlamm25@gmail.com` with subject prefix `[FAMP SECURITY]`.

Please include:

- A clear description of the issue and its impact (interop, non-repudiation,
  authentication, denial of service, etc.).
- Reproduction steps. For canonicalization or signature issues, the
  smallest possible failing test vector (raw bytes, public keys, expected
  vs. actual output) is the most useful artifact.
- Affected versions / commit SHAs.
- Any suggested mitigation, if known.

You should receive an acknowledgement within **5 business days**. If you
do not, please re-send via the alternate channel above.

## Scope

In scope:

- All crates under `crates/` in this repository.
- The reference HTTP transport (`famp-transport-http`).
- Canonical JSON (`famp-canonical`) — RFC 8785 conformance bugs that
  produce divergent bytes are in scope even if no signature is involved,
  because they break interop.
- Ed25519 sign/verify (`famp-crypto`) — including domain-separation
  prefix handling, weak-key acceptance, and `verify_strict` bypass paths.
- The five-state task FSM (`famp-fsm`) — including reachability of
  states or transitions outside the spec.
- TOFU keyring (`famp-keyring`) — fingerprint pinning, peer parsing,
  trust-bootstrap edge cases.
- The MCP server surface exposed by `famp mcp`.

Out of scope:

- Bugs in third-party dependencies (please report upstream; we monitor
  `cargo audit`).
- Issues in scaffolding scripts under `scripts/` that do not affect the
  protocol library or daemon.
- Reports that require physical access to the machine running a daemon
  (the local-first profile explicitly trusts the local filesystem).
- Theoretical concerns without a reproduction or failing test vector.

## Disclosure Policy

We follow coordinated disclosure:

1. Reporter contacts us privately.
2. We confirm the issue and develop a fix.
3. We publish a fix release, then a security advisory crediting the
   reporter (unless the reporter prefers anonymity).

For canonicalization or signature-verification bugs, expect a fast
turnaround — these are blocking issues for the project's core claim.

## Hardening

The implementation is intentionally strict:

- `verify_strict` (not `verify`) is the only Ed25519 verification path
  exposed publicly; raw `verify` is unreachable from outside the crate.
- A 12-byte `FAMP-sig-v1\0` domain-separation prefix is prepended
  internally to every signature input; callers cannot construct signing
  input by hand.
- `unsafe_code = "forbid"` is set on every crate in the workspace.
- RFC 8785 (canonical JSON) and RFC 8032 (Ed25519) external test vectors
  are wired into CI as blocking gates; signature divergence fails the
  build.
- Constant-time comparisons (`subtle::ConstantTimeEq`) are used for key
  and signature equality checks.

If you find a code path that violates any of the above, that is itself
considered a security issue worth reporting.
