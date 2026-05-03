#!/usr/bin/env bash
# ~/.famp/hook-runner.sh — FAMP HOOK-04b execution runner
# Stop-hook shim. Reads stdin JSON, parses transcript_path, glob-matches
# rows in ~/.famp-local/hooks.tsv, fires `famp send` once per matching row.
# CRITICAL: This script MUST NEVER fail the Stop hook. All paths exit 0.
set -uo pipefail

LOG="${HOME}/.famp/hook-runner.log"
HOOKS_TSV="${HOME}/.famp-local/hooks.tsv"
mkdir -p "${HOME}/.famp" 2>/dev/null || true

log() { printf '[%s] %s\n' "$(date -u +%FT%TZ)" "$*" >> "$LOG" 2>/dev/null || true; }

# 1. Read stdin JSON. If absent or malformed, log + exit 0.
STDIN_JSON="$(cat)" || { log "no stdin"; exit 0; }

# 2. Extract transcript_path. Use python3 (always present on macOS) for JSON;
#    avoids hard dep on jq.
TRANSCRIPT="$(printf '%s' "$STDIN_JSON" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("transcript_path",""))' \
    2>/dev/null)" || { log "no transcript_path"; exit 0; }
[ -n "$TRANSCRIPT" ] && [ -f "$TRANSCRIPT" ] || { log "transcript_path missing: $TRANSCRIPT"; exit 0; }

# 3. Extract file paths edited in the LAST assistant turn. Walk transcript
#    JSONL; find latest assistant boundary; collect Edit/Write/MultiEdit
#    file_path arguments. Dedup. Empty list → exit 0 silently.
FILES="$(python3 - "$TRANSCRIPT" <<'PY' 2>/dev/null || true
import json, sys
path = sys.argv[1]
last_turn_files = set()
with open(path) as f:
    for line in f:
        try:
            ev = json.loads(line)
        except Exception:
            continue
        if ev.get("role") == "user":
            last_turn_files.clear()  # new turn boundary
        for block in ev.get("content") or []:
            if isinstance(block, dict) and block.get("type") == "tool_use":
                name = block.get("name", "")
                if name in ("Edit", "Write", "MultiEdit"):
                    fp = (block.get("input") or {}).get("file_path")
                    if fp:
                        last_turn_files.add(fp)
print("\n".join(sorted(last_turn_files)))
PY
)"
[ -n "$FILES" ] || { log "no edited files in last turn"; exit 0; }

# 4. Read hooks.tsv. Format: <id>\t<event>:<glob>\t<to>\t<added_at>.
[ -r "$HOOKS_TSV" ] || { log "no hooks.tsv at $HOOKS_TSV"; exit 0; }

# 5. For each row, glob-match against the file list; fire ONE `famp send` per
#    matching row (D-07: not per-file). Log + continue on any failure.
while IFS=$'\t' read -r id spec to _ts; do
    [ -n "$id" ] && [ "${id#\#}" = "$id" ] || continue   # skip blank/comment
    event="${spec%%:*}"
    glob="${spec#*:}"
    [ "$event" = "Edit" ] || continue                    # only Edit-class hooks for v0.9
    matched=0
    while IFS= read -r f; do
        # shellcheck disable=SC2254  # intentional glob expansion in pattern
        case "$f" in
            $glob) matched=1; break ;;
        esac
    done <<< "$FILES"
    if [ "$matched" = 1 ]; then
        log "match id=$id glob=$glob to=$to — dispatching famp send"
        famp send --to "$to" --new-task "Edit hook: $glob matched in last turn" \
            >> "$LOG" 2>&1 || log "famp send failed for $id (suppressed)"
    fi
done < "$HOOKS_TSV"

exit 0
