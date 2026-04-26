---
name: Bug report
about: Report a defect in the FAMP reference implementation
title: "[bug] "
labels: bug
assignees: ''
---

<!--
Thanks for taking the time to report a bug. FAMP is a protocol
implementation, so reproducibility hinges on byte-exact inputs. The
fields below are tuned for the kinds of bugs we see most often
(canonicalization, signatures, FSM transitions). Fill in what
applies; delete the rest.
-->

## Environment

- **FAMP version / git SHA:** <!-- e.g. v0.8.0 / `git rev-parse HEAD` -->
- **Affected crate(s):** <!-- e.g. famp-canonical, famp-crypto, famp-core, famp-fsm, famp-envelope, famp-transport-http -->
- **Rust toolchain:** <!-- output of `rustc -V` -->
- **OS / arch:** <!-- e.g. macOS 14.5 arm64, Ubuntu 24.04 x86_64 -->
- **Transport:** <!-- in-process MemoryTransport / HTTPS / scripts/famp-local / other -->

## Summary

<!-- One or two sentences: what went wrong, what you expected. -->

## Reproduction steps

1.
2.
3.

A minimal failing test (added under the affected crate's `tests/` or
as a `#[test]` in the relevant module) is the gold standard. Even a
shell snippet that drives `famp-local` or the `famp` CLI is very
helpful.

## Category-specific details

<!-- Keep the section that applies; delete the others. -->

### Canonicalization bug (RFC 8785 / `famp-canonical`)

- **Input JSON** (raw, exactly as fed to the canonicalizer):

  ```json
  ```

- **Expected canonical bytes** (hex or base64url-unpadded):
- **Actual canonical bytes:**
- **Diff** (first differing byte offset, both bytes in hex):

### Signature bug (Ed25519 / `famp-crypto` / envelope verification)

- **Public key** (base64url-unpadded, 32 bytes raw):
- **Signature** (base64url-unpadded, 64 bytes raw):
- **Payload bytes** (base64url-unpadded, exactly what was signed/verified):
- **Domain separation prefix used:** <!-- expected `FAMP-sig-v1\0` per v0.5.1 -->
- **`verify_strict` vs `verify`:** <!-- which path failed/succeeded -->

### FSM bug (`famp-fsm`)

- **Starting state:** <!-- REQUESTED / COMMITTED / COMPLETED / FAILED / CANCELLED -->
- **Attempted transition:** <!-- event + target state -->
- **Expected outcome:** <!-- accepted / rejected with which error -->
- **Actual outcome:**

### Transport bug (`famp-transport-http`)

- **Request line + relevant headers** (redact tokens):
- **Response status + body:**
- **TLS context** (rustls/native, trust anchors used):

## Logs / output

<!--
Paste relevant log lines. If the bug surfaced from a test, include the
nextest output for that test. Wrap in ``` blocks.
-->

```
```

## Severity / impact

- [ ] Breaks signature verification or canonicalization (interop blocker — high priority)
- [ ] Breaks an FSM invariant
- [ ] Breaks transport but interop primitives are intact
- [ ] Cosmetic / doc / DX issue

## Anything else

<!-- Workarounds tried, links to related issues, suspected fix area. -->
