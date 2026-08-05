# Phase 20 Clean-Host Rehearsal Record — BLANK TEMPLATE

Copy this file to `20-REHEARSAL.md` only when a real supported clean host is
available. Every evidence value is owner-attributed, UTC-timestamped, and
redacted. Never record invite codes, private keys, authentication tokens, raw
transcripts, or unredacted home paths. A product/guide failure requires repair
and reset; an invalid run requires a fully clean rerun.

Each row represents: criterion, owner, capture command/attestation, UTC time,
redacted evidence, result. Replace every `<REQUIRED>` value; do not edit keys.

```text
outcome=unresolved
redaction_review=<REQUIRED>
redaction_findings=<REQUIRED>
clean_preflight=<REQUIRED>
clean_owner=<REQUIRED>
clean_utc=<REQUIRED>
clean_os_arch=<REQUIRED>
release_famp_version=<REQUIRED>
release_gateway_version=<REQUIRED>
pairing_ready=<REQUIRED>
task_a_id=<REQUIRED>
task_a_owner=<REQUIRED>
task_a_utc=<REQUIRED>
task_a_state=<REQUIRED>
task_b_id=<REQUIRED>
task_b_owner=<REQUIRED>
task_b_utc=<REQUIRED>
task_b_state=<REQUIRED>
```

Exactly one final outcome is permitted: `pass`, `product_or_guide_failure`, or
`invalid`. The template deliberately defaults to none of them.
