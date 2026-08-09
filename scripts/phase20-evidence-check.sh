#!/bin/sh
# Validate the shape of a populated Phase 20 record; never supplies evidence.
set -eu

[ "$#" -eq 2 ] || { echo "usage: $0 <rehearsal|acceptance> <record>" >&2; exit 2; }
MODE=$1
RECORD=$2
[ -f "$RECORD" ] || { echo "record not found" >&2; exit 2; }
case "$MODE" in rehearsal|acceptance) ;; *) echo "unknown mode: $MODE" >&2; exit 2 ;; esac

FAIL=0
require() {
    key=$1
    count=$(grep -c "^${key}=" "$RECORD" || true)
    if [ "$count" -ne 1 ]; then echo "invalid field count: $key" >&2; FAIL=1; return; fi
    value=$(grep "^${key}=" "$RECORD" | sed "s/^${key}=//")
    case "$value" in ''|*'<REQUIRED>'*|unresolved) echo "incomplete field: $key" >&2; FAIL=1 ;; esac
}

OUTCOMES=$(grep -c '^outcome=' "$RECORD" || true)
[ "$OUTCOMES" -eq 1 ] || { echo "exactly one outcome required" >&2; FAIL=1; }
OUTCOME=$(grep '^outcome=' "$RECORD" 2>/dev/null | sed 's/^outcome=//' || true)
case "$OUTCOME" in pass|product_or_guide_failure|invalid) ;; *) echo "unknown outcome" >&2; FAIL=1 ;; esac

# Redaction is checked for EVERY outcome: a run that failed or was
# invalidated still produced notes, and those can leak just as easily as a
# passing record's.
for key in redaction_review redaction_findings; do require "$key"; done
[ "$(grep '^redaction_review=' "$RECORD" | sed 's/.*=//' || true)" = pass ] || FAIL=1
[ "$(grep '^redaction_findings=' "$RECORD" | sed 's/.*=//' || true)" = none ] || FAIL=1

# Everything below this point describes a run that got far enough to produce
# it. Demanding it of a `product_or_guide_failure` or `invalid` record made
# the two most likely outcomes of any attempt literally unrecordable: the
# run dies at the step that failed, so every later field is honestly still
# `<REQUIRED>`, and `require` rejected exactly that. A failed attempt instead
# has to say where it died and what happened -- which is the evidence that
# matters for those outcomes.
if [ "$OUTCOME" != pass ]; then
    for key in failure_stage failure_detail; do require "$key"; done
    if [ "$FAIL" -eq 0 ]; then
        echo "EVIDENCE RECORD: VALID $MODE $OUTCOME"
        exit 0
    fi
    exit 1
fi

for key in task_a_id task_a_owner task_a_utc task_a_state task_b_id task_b_owner task_b_utc task_b_state; do require "$key"; done

TASK_A=$(grep '^task_a_id=' "$RECORD" | sed 's/.*=//' || true)
TASK_B=$(grep '^task_b_id=' "$RECORD" | sed 's/.*=//' || true)
[ -n "$TASK_A" ] && [ "$TASK_A" != "$TASK_B" ] || { echo "task IDs must be distinct" >&2; FAIL=1; }
for key in task_a_state task_b_state; do
    state=$(grep "^${key}=" "$RECORD" | sed 's/.*=//' || true)
    case "$state" in COMPLETED|FAILED|CANCELLED) ;; *) echo "nonterminal receiver state: $key" >&2; FAIL=1 ;; esac
done

if [ "$MODE" = rehearsal ]; then
    for key in clean_preflight clean_owner clean_utc clean_os_arch release_famp_version release_gateway_version pairing_ready; do require "$key"; done
else
    for key in independent_machines different_networks shared_vpn copied_keys question_log no_coaching guide_commit guide_digest ben_owner ben_utc ben_os_arch ben_famp_version ben_gateway_version follower_owner follower_utc follower_os_arch follower_famp_version follower_gateway_version; do require "$key"; done
    for expected in independent_machines=yes different_networks=yes shared_vpn=no copied_keys=no no_coaching=yes; do
        grep -qx "$expected" "$RECORD" || { echo "hard precondition failed: $expected" >&2; FAIL=1; }
    done
    for key in ben_os_arch follower_os_arch; do grep -q "^${key}=REDACTED:" "$RECORD" || { echo "unredacted machine evidence: $key" >&2; FAIL=1; }; done
    n=1
    while [ "$n" -le 7 ]; do
        for suffix in text owner utc first_paraphrase judgment; do require "message_${n}_${suffix}"; done
        n=$((n + 1))
    done
fi

if grep -Eq '(/Users/|/home/[^ <]+|private[_ -]?key|invite[_ -]?code|auth[_ -]?token|token=)' "$RECORD"; then
    echo "redaction review required: forbidden secret/path pattern" >&2
    FAIL=1
fi

[ "$FAIL" -eq 0 ] || exit 1
echo "EVIDENCE RECORD: VALID $MODE $OUTCOME"
