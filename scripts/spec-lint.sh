#!/usr/bin/env bash
# spec-lint.sh — FAMP v0.5.1 spec anchor lint.
#
# Runs ripgrep anchor checks for every SPEC-xx requirement defined in
# .planning/phases/01-spec-fork-v0-5-1/01-VALIDATION.md. Each check greps
# FAMP-v0.5.1-spec.md for a stable textual anchor. A failing anchor means a
# requirement has not yet been written into the spec file.
#
# Exit code: number of failed anchors (0 = green).
#
# NOTE: This script is expected to fail until Waves 1-3 of phase
# 01-spec-fork-v0-5-1 populate the spec. Only SPEC-20 (the version constant
# from Wave 0 Plan 01-01) is guaranteed to pass immediately.

set -uo pipefail

SPEC="FAMP-v0.5.1-spec.md"
FAILED=0
PASSED=0

if [[ ! -f "$SPEC" ]]; then
  echo "[FAIL] spec file not found: $SPEC" >&2
  exit 1
fi

check() {
  local id="$1"
  local desc="$2"
  shift 2
  if "$@" >/dev/null 2>&1; then
    echo "[PASS] $id" >&2
    PASSED=$((PASSED + 1))
  else
    echo "[FAIL] $id: $desc" >&2
    FAILED=$((FAILED + 1))
  fi
}

check SPEC-01 "v0.5.1 Changelog heading"        rg -q 'v0.5.1 Changelog'               "$SPEC"
check SPEC-02 "RFC 8785 citation"                rg -q 'RFC 8785'                       "$SPEC"
check SPEC-03 "FAMP-sig-v1 domain separator"     rg -q 'FAMP-sig-v1'                    "$SPEC"
check SPEC-04 "recipient anti-replay binding"    rg -q 'recipient.{0,20}anti-replay|binds.{0,10}`to`' "$SPEC"
check SPEC-05 "federation_credential field"      rg -q 'federation_credential'          "$SPEC"

# SPEC-06 — both card_version and min_compatible_version anchors required.
if rg -q 'card_version' "$SPEC" && rg -q 'min_compatible_version' "$SPEC"; then
  echo "[PASS] SPEC-06" >&2
  PASSED=$((PASSED + 1))
else
  echo "[FAIL] SPEC-06: card_version && min_compatible_version" >&2
  FAILED=$((FAILED + 1))
fi

# SPEC-07 — both ±60 and 300 seconds anchors required.
if rg -q '±60' "$SPEC" && rg -q '300.{0,10}seconds' "$SPEC"; then
  echo "[PASS] SPEC-07" >&2
  PASSED=$((PASSED + 1))
else
  echo "[FAIL] SPEC-07: ±60 && 300 seconds" >&2
  FAILED=$((FAILED + 1))
fi

check SPEC-08 "idempotency 128-bit"              rg -q 'idempotency.{0,30}128-bit'      "$SPEC"
check SPEC-09 "ack disposition terminal"         rg -q 'ack.disposition.{0,50}terminal' "$SPEC"
check SPEC-10 "envelope-level whitelist / FSM inspects" rg -q 'envelope-level.{0,20}whitelist|FSM.{0,20}inspects' "$SPEC"
check SPEC-11 "transfer timeout tiebreak"        rg -q 'transfer.{0,10}timeout.{0,20}tiebreak' "$SPEC"
check SPEC-12 "EXPIRED vs deliver"               rg -q 'EXPIRED.{0,20}deliver'          "$SPEC"
check SPEC-13 "conditional lapse"                rg -q 'conditional.{0,10}lapse'        "$SPEC"
check SPEC-14 "COMMITTED_PENDING_RESOLUTION"     rg -q 'COMMITTED_PENDING_RESOLUTION'   "$SPEC"
check SPEC-15 "supersession round"               rg -q 'supersession.{0,30}round'       "$SPEC"
check SPEC-16 "capability snapshot commit-time"  rg -q 'capability.{0,20}snapshot.{0,20}commit-time' "$SPEC"

# SPEC-17 — one body-schema anchor per message kind.
SPEC17_OK=1
for b in commit propose deliver control delegate; do
  if ! rg -q "\`$b\` body" "$SPEC"; then
    echo "[FAIL] SPEC-17: missing \`$b\` body anchor" >&2
    SPEC17_OK=0
  fi
done
if [[ "$SPEC17_OK" -eq 1 ]]; then
  echo "[PASS] SPEC-17" >&2
  PASSED=$((PASSED + 1))
else
  FAILED=$((FAILED + 1))
fi

check SPEC-18 "sha256:<hex> artifact scheme"     rg -q 'sha256:<hex>'                   "$SPEC"
check SPEC-19 "unpadded base64url"               rg -q 'unpadded base64url'             "$SPEC"
check SPEC-20 "FAMP_SPEC_VERSION constant"       rg -q 'FAMP_SPEC_VERSION\s*=\s*"0\.5\.1"' "$SPEC"

# SPEC-01-FULL — strict count of Δnn changelog entries. Populated by Plan 06.
CHG_COUNT=$(rg -c '^v0\.5\.1-Δ' "$SPEC" 2>/dev/null || echo 0)
if [[ "$CHG_COUNT" -ge 20 ]]; then
  echo "[PASS] SPEC-01-FULL ($CHG_COUNT entries)" >&2
  PASSED=$((PASSED + 1))
else
  echo "[FAIL] SPEC-01-FULL: need ≥20 v0.5.1-Δnn entries, found $CHG_COUNT" >&2
  FAILED=$((FAILED + 1))
fi

echo "" >&2
echo "spec-lint: $PASSED passed, $FAILED failed" >&2
exit "$FAILED"
