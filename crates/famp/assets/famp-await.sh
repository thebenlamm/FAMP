#!/usr/bin/env bash
# ~/.claude/hooks/famp-await.sh — FAMP inbound listen-mode Stop hook (v0.9)
#
# Activates when the session transcript contains the most recent successful
# famp_register call with listen:true. Blocks on
# `famp await --as <identity> --timeout 23h`. On message, emits a
# notification-only {"decision":"block","reason":"..."} so Claude calls
# famp_inbox to retrieve the content — peer bytes never touch `reason`.
#
# Exit 0 always (fail-open): never trap Claude in a session.
#
# Note: uses a Python temp-file approach rather than heredocs for the
# identity-extraction script, to remain compatible with bash 3.2 (macOS
# default). Bash 3.2 mis-parses <<'DELIM' inside $() when running script
# files (not -c strings) — a known 3.2 limitation fixed in bash 4.x.
set -uo pipefail

# --- Read transcript_path from stdin BEFORE redirecting stdin -----------
STDIN_JSON="$(cat 2>/dev/null || true)"
TRANSCRIPT="$(printf '%s' "$STDIN_JSON" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("transcript_path",""))' \
    2>/dev/null || true)"

# Disconnect stdin now to avoid SIGPIPE during the long await block.
exec 0</dev/null

# --- Logging -----------------------------------------------------------
STATE_DIR="${XDG_STATE_HOME:-${HOME:-/tmp}/.local/state}/famp"
LOG_FILE="${FAMP_HOOK_LOG:-$STATE_DIR/await-hook.log}"
mkdir -p "$(dirname "$LOG_FILE")" 2>/dev/null || true
[ -L "$LOG_FILE" ] && LOG_FILE=/dev/null
log() { printf '[%s pid=%s] %s\n' "$(date -Iseconds)" "$$" "$*" >> "$LOG_FILE" 2>/dev/null || true; }
log "hook invoked"

# --- Transcript gate ---------------------------------------------------
if [ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ]; then
    log "no transcript_path; exiting no-op"
    exit 0
fi

# --- Write identity-extraction Python to a temp file -------------------
# Avoids heredoc-in-$() which bash 3.2 (macOS) cannot parse from script files.
# Uses cat with a bare (non-subshell) heredoc, which bash 3.2 handles fine.
PY_SCRIPT="$(mktemp "${TMPDIR:-/tmp}/famp-extract-XXXXXX")" || PY_SCRIPT=""
if [ -z "$PY_SCRIPT" ]; then
    log "mktemp failed for python script; exiting no-op"
    exit 0
fi

cat > "$PY_SCRIPT" << 'PYEOF'
import json, os, sys

path = sys.argv[1]
regs    = []   # (line_pos, tool_use_id, identity)
results = {}   # tool_use_id -> ok (bool)

# Limit scan to last 2 MB to protect against huge transcripts.
MAX_BYTES = 2_000_000
fsize  = os.path.getsize(path)
offset = max(0, fsize - MAX_BYTES)

pos = 0
# Known limitation: a later famp_register(listen:false) in the same session
# does not cancel listen mode — only the most recent listen-active register
# (absent OR true) is tracked. Dedicated listen windows never re-register,
# so this is acceptable.
with open(path, encoding='utf-8', errors='replace') as f:
    if offset > 0:
        f.seek(offset)
        f.readline()  # discard partial line at seek boundary
    for line in f:
        pos += 1
        try:
            ev = json.loads(line)
        except Exception:
            continue
        msg = ev.get("message") if isinstance(ev.get("message"), dict) else ev
        content = msg.get("content") or []
        if isinstance(content, str):
            continue
        for block in content:
            if not isinstance(block, dict):
                continue
            t    = block.get("type", "")
            name = str(block.get("name", ""))
            if t == "tool_use":
                if name.endswith("famp_register"):
                    inp = block.get("input") or {}
                    # Fix 1 (2026-05-12): listen defaults to ON when the
                    # field is absent or JSON null, mirroring the MCP
                    # tool default at register.rs:80 (unwrap_or(true)).
                    # Only an explicit JSON `false` suppresses listen.
                    # `inp.get("listen") is not False` returns True for
                    # missing key (None) and True for JSON true; only
                    # JSON false makes it False.
                    if inp.get("listen") is not False:
                        ident = inp.get("identity") or inp.get("name", "")
                        uid   = block.get("id", "")
                        if ident and uid:
                            regs.append((pos, uid, ident))
            elif t == "tool_result":
                uid = block.get("tool_use_id", "")
                # Strict boolean check: only JSON true (Python True) counts as error.
                # Avoids bool("false") == True for string-typed is_error values.
                results[uid] = block.get("is_error") is not True

# Find the most recent listen registration that succeeded.
# famp_leave is a channel operation, not an unregister — we do not track it.
active = ""
for _, uid, ident in reversed(regs):
    if not results.get(uid, False):
        continue
    active = ident
    break

print(active)
PYEOF

# --- Extract identity from transcript ---------------------------------
ACTIVE_IDENTITY="$(python3 "$PY_SCRIPT" "$TRANSCRIPT" 2>/dev/null || true)"
rm -f "$PY_SCRIPT"

if [ -z "$ACTIVE_IDENTITY" ]; then
    log "no listen registration in transcript; exiting no-op"
    exit 0
fi

# --- Validate identity (belt-and-suspenders after Python extraction) ---
# Reject embedded newlines first — grep -E anchors (^$) match per-line, so
# a multi-line value like "dk\nmalicious" would otherwise pass validation.
case "$ACTIVE_IDENTITY" in
    *$'\n'*) log "identity contains newline; exiting no-op"; exit 0 ;;
esac
if ! printf '%s' "$ACTIVE_IDENTITY" | grep -qE '^[A-Za-z0-9._-]{1,64}$'; then
    log "invalid identity from transcript: $ACTIVE_IDENTITY; exiting no-op"
    exit 0
fi

FAMP_BIN="$(command -v famp 2>/dev/null || echo "$HOME/.cargo/bin/famp")"
log "listen mode active: identity=$ACTIVE_IDENTITY bin=$FAMP_BIN"

# --- Block on inbox ---------------------------------------------------
ERR_FILE="$(mktemp "${TMPDIR:-/tmp}/famp-await-err.XXXXXX")" || ERR_FILE=""
if [ -n "$ERR_FILE" ]; then
    MSG=$("$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h 2>"$ERR_FILE")
    STATUS=$?
    ERR=$(cat "$ERR_FILE" 2>/dev/null || true)
    rm -f "$ERR_FILE"
else
    MSG=$("$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h 2>&1)
    STATUS=$?
    ERR="$MSG"
fi
log "await returned status=$STATUS msg_bytes=${#MSG}"
[ -n "$ERR" ] && log "stderr: $ERR"

# --- 64KB cap + UTF-8 sanitization (spec security requirement) --------
if [ "${#MSG}" -gt 65536 ]; then
    MSG="${MSG:0:65536}"
    log "envelope truncated to 64KB"
fi
if command -v iconv >/dev/null 2>&1; then
    MSG="$(printf '%s' "$MSG" | iconv -f UTF-8 -t UTF-8 -c 2>/dev/null || printf '%s' "$MSG")"
fi

# --- Backup received envelope -----------------------------------------
if [ -n "${MSG//[[:space:]]/}" ]; then
    BACKUP_DIR="$STATE_DIR/received"
    if mkdir -p "$BACKUP_DIR" 2>/dev/null; then
        TS=$(date +%Y%m%dT%H%M%S)
        FNAME="${TS}-$$-$RANDOM.jsonl"
        printf '%s\n' "$MSG" > "$BACKUP_DIR/$FNAME" 2>/dev/null \
            && log "envelope backed up: $BACKUP_DIR/$FNAME"
    fi
fi

# --- Error / empty handling -------------------------------------------
if [ $STATUS -ne 0 ]; then
    log "await non-zero exit (status=$STATUS, msg_bytes=${#MSG}); fail-open exit 0"
    exit 0
fi

if [ -z "${MSG//[[:space:]]/}" ]; then
    log "await timeout or empty; clean stop"
    exit 0
fi

# --- Extract sender for notification string ---------------------------
# Best-effort: parse `from` field from the envelope JSON.
SENDER="$(python3 -c '
import json, sys
try:
    env = json.loads(sys.argv[1])
    print(env.get("from", env.get("sender", "unknown")))
except Exception:
    print("unknown")
' "$MSG" 2>/dev/null || echo "unknown")"

# Validate sender — reject anything outside printable word/punct chars.
if ! printf '%s' "$SENDER" | grep -qE '^[A-Za-z0-9@._:/-]{1,128}$'; then
    log "sender failed validation; using 'unknown'"
    SENDER="unknown"
fi

# --- Emit notification-only block decision ----------------------------
# SECURITY: peer-controlled envelope bytes are NOT included in reason.
# The agent calls famp_inbox to retrieve the actual content.
if ! command -v jq >/dev/null 2>&1; then
    log "jq not found; cannot emit block decision"
    exit 0
fi

REASON="[FAMP listen mode] New message from ${SENDER}. Call famp_inbox to read it."
OUT=$(jq -n --arg r "$REASON" '{decision: "block", reason: $r}')
log "emitting block decision (${#OUT} bytes); sender=$SENDER"
printf '%s\n' "$OUT"
