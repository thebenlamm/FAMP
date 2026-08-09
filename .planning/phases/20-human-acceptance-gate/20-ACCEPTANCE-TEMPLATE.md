# Phase 20 Second-Person Acceptance Record — BLANK TEMPLATE

Copy this file to `20-ACCEPTANCE.md` only for the genuine event. Evidence must
be supplied by the owner of the observed machine or received task, with UTC
capture time and redaction. Never include pairing codes, signing-key material,
credentials, raw transcripts, or unredacted home paths. Ben may not
type, screen-control, copy keys, or coach the follower through a missing step.

Each evidence row encodes criterion, owner, capture command/attestation, UTC
timestamp, redacted evidence, and result. Replace every `<REQUIRED>` value.

```text
outcome=unresolved
failure_stage=<REQUIRED>
failure_detail=<REQUIRED>
redaction_review=<REQUIRED>
redaction_findings=<REQUIRED>
independent_machines=<REQUIRED>
different_networks=<REQUIRED>
shared_vpn=<REQUIRED>
copied_keys=<REQUIRED>
question_log=<REQUIRED>
no_coaching=<REQUIRED>
guide_commit=<REQUIRED>
guide_digest=<REQUIRED>
ben_owner=<REQUIRED>
ben_utc=<REQUIRED>
ben_os_arch=<REQUIRED>
ben_famp_version=<REQUIRED>
ben_gateway_version=<REQUIRED>
follower_owner=<REQUIRED>
follower_utc=<REQUIRED>
follower_os_arch=<REQUIRED>
follower_famp_version=<REQUIRED>
follower_gateway_version=<REQUIRED>
task_a_id=<REQUIRED>
task_a_owner=<REQUIRED>
task_a_utc=<REQUIRED>
task_a_state=<REQUIRED>
task_b_id=<REQUIRED>
task_b_owner=<REQUIRED>
task_b_utc=<REQUIRED>
task_b_state=<REQUIRED>
message_1_text=That does not look like a pairing code. A pairing code is exactly five lowercase words separated by spaces. Check the message you were sent and type it again.
message_1_owner=<REQUIRED>
message_1_utc=<REQUIRED>
message_1_first_paraphrase=<REQUIRED>
message_1_judgment=<REQUIRED>
message_2_text=This code has expired. Codes last 24 hours. Ask the person who invited you to send a new one.
message_2_owner=<REQUIRED>
message_2_utc=<REQUIRED>
message_2_first_paraphrase=<REQUIRED>
message_2_judgment=<REQUIRED>
message_3_text=This code has already been used. If that was not you, tell the person who invited you right away and ask them to run: famp pair revoke --all-pending
message_3_owner=<REQUIRED>
message_3_utc=<REQUIRED>
message_3_first_paraphrase=<REQUIRED>
message_3_judgment=<REQUIRED>
message_4_text=Too many wrong tries, so this code is now locked. Ask the person who invited you to send a new one.
message_4_owner=<REQUIRED>
message_4_utc=<REQUIRED>
message_4_first_paraphrase=<REQUIRED>
message_4_judgment=<REQUIRED>
message_5_text=That code did not match. Check for a typo, then try again. If you run out of tries, ask the person who invited you to send a new code.
message_5_owner=<REQUIRED>
message_5_utc=<REQUIRED>
message_5_first_paraphrase=<REQUIRED>
message_5_judgment=<REQUIRED>
message_6_text=Could not reach {url}. Check that you copied the address exactly, then ask the person who invited you whether their FAMP gateway is running.
message_6_owner=<REQUIRED>
message_6_utc=<REQUIRED>
message_6_first_paraphrase=<REQUIRED>
message_6_judgment=<REQUIRED>
message_7_text=This code cannot be redeemed on the same machine that created it. Run this on the machine you want to connect.
message_7_owner=<REQUIRED>
message_7_utc=<REQUIRED>
message_7_first_paraphrase=<REQUIRED>
message_7_judgment=<REQUIRED>
```

The follower's first response is recorded before any explanation. Human
judgment remains open until the reviewer classifies all seven responses.
Exactly one outcome is allowed: `pass`, `product_or_guide_failure`, or
`invalid`. Invalid means a fully clean rerun; product/guide failure means
repair and reset, never coaching-through.
