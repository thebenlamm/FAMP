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

FAMP_BIN="$(command -v famp 2>/dev/null || echo "$HOME/.cargo/bin/famp")"

if [ -z "$ACTIVE_IDENTITY" ]; then
    # --- Broker fallback (compaction resilience, Fix A 260721) ---------
    # A long session whose transcript was compacted (Claude Code /compact)
    # can lose its famp_register marker out of the 2 MB scan tail above,
    # leaving no identity even though the broker still holds a live
    # listen=true registration for this session. Resolve it from the broker
    # by matching listen_mode==true AND this session's cwd, parsed from
    # `famp inspect identities --json` (robust against spaces in paths;
    # supersedes the fragile awk-on-table approach). This only runs when
    # the transcript yielded nothing, so it adds no cost to normal Stops.
    #
    # Three outcomes, all FAIL-OPEN:
    #   exactly 1 listen=true row for this cwd -> adopt it (silent self-heal).
    #   >=2 (ambiguous)                        -> Fix E: surface the disarm.
    #   0                                       -> nothing to listen for; no-op.
    SESSION_CWD="$(printf '%s' "$STDIN_JSON" \
        | python3 -c 'import json,sys; print(json.load(sys.stdin).get("cwd",""))' \
        2>/dev/null || true)"
    [ -n "$SESSION_CWD" ] || SESSION_CWD="$PWD"

    # Emits the unique listen=true identity for this cwd, or the sentinel
    # "!AMBIGUOUS" (the '!' is outside the identity charset, so it can never
    # be mistaken for a real name), or "" for no match / any error.
    FALLBACK="$("$FAMP_BIN" inspect identities --json 2>/dev/null \
        | python3 -c 'import json, sys
cwd = sys.argv[1]
try:
    data = json.load(sys.stdin)
except Exception:
    sys.exit(0)
rows = data.get("rows", []) if isinstance(data, dict) else []
m = [r.get("name", "") for r in rows
     if isinstance(r, dict) and r.get("listen_mode") is True
     and r.get("cwd") == cwd and r.get("name")]
print(m[0] if len(m) == 1 else ("!AMBIGUOUS" if len(m) >= 2 else ""))' \
        "$SESSION_CWD" 2>/dev/null || true)"

    if [ "$FALLBACK" = "!AMBIGUOUS" ]; then
        # --- Fix E (260721): surface the disarm instead of silent no-op --
        # >=2 listen=true identities share this cwd, so we cannot safely
        # auto-adopt, but the broker clearly shows this session SHOULD be
        # listening. Emit a visible block warning ONCE (marker keyed by
        # transcript) so the agent/human learns immediately rather than via
        # a missed message hours later. With Fix B (idempotent
        # self-re-register) the suggested recovery actually works now.
        WARN_MARK="$STATE_DIR/disarm-warned/$(basename "$TRANSCRIPT" 2>/dev/null || printf 'default')"
        if [ -f "$WARN_MARK" ]; then
            log "listen-mode disarm already surfaced this session (ambiguous cwd=$SESSION_CWD); no-op"
            exit 0
        fi
        mkdir -p "$(dirname "$WARN_MARK")" 2>/dev/null || true
        : > "$WARN_MARK" 2>/dev/null || true
        if command -v jq >/dev/null 2>&1; then
            WREASON="[FAMP listen mode] Auto-wake appears DISARMED (likely after a /compact): the broker shows multiple listen=true identities in this directory, so the Stop hook cannot tell which one is yours. Re-register (famp_register with your identity and listen:true) or restart this window to re-arm."
            jq -n --arg r "$WREASON" '{decision: "block", reason: $r}'
            log "surfaced listen-mode disarm warning (ambiguous cwd=$SESSION_CWD)"
            exit 0
        fi
        log "listen-mode disarm detected (ambiguous) but jq missing; no-op"
        exit 0
    elif [ -n "$FALLBACK" ]; then
        ACTIVE_IDENTITY="$FALLBACK"
        log "transcript had no register; broker fallback resolved identity=$ACTIVE_IDENTITY (cwd=$SESSION_CWD)"
    fi
fi

if [ -z "$ACTIVE_IDENTITY" ]; then
    log "no listen registration in transcript (and no unique broker fallback); exiting no-op"
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

log "listen mode active: identity=$ACTIVE_IDENTITY bin=$FAMP_BIN"

# --- Cancellation seam for issue #21 (host input-queue watcher) --------
# A blocked Stop hook keeps the turn alive so an inbound FAMP message can
# wake the agent — that block is the wake mechanism and must stay. The bug
# is that the block is uncancellable: while it blocks, the host never
# drains its input queue, so background-agent completion notifications sit
# there until the user hits Esc.
#
# Fix: a background watcher writes one byte to fd 9 when the Claude Code
# input queue has outstanding input (the most recent JSON-parsed
# queue-operation record is an `enqueue`). `famp await --abort-on-fd 9`
# then returns exit 3; we exit 0; the host drains. Listen mode self-heals
# — the next turn's Stop hook re-arms the await.
#
# Everything here is FAIL-OPEN: any setup/watcher/python error means we run
# a plain `famp await` exactly as before and NEVER abort on uncertainty.
# `famp` itself knows nothing about Claude Code — all host coupling is here.
ABORT_FD_READY=""
QWATCH_DIR=""
QWATCH_FIFO=""
QWATCH_PY=""
QWATCH_PID=""

# fd 9 opened read-write (<>): opening neither blocks for a peer nor reports
# spurious EOF, and bash does not set CLOEXEC on exec-redirected fds, so
# `famp await --abort-on-fd 9` inherits it.
QWATCH_DIR="$(mktemp -d "${TMPDIR:-/tmp}/famp-qwatch-XXXXXX" 2>/dev/null || true)"
if [ -n "$QWATCH_DIR" ]; then
    QWATCH_FIFO="$QWATCH_DIR/abort.fifo"
    QWATCH_PY="$QWATCH_DIR/qwatch.py"
    if mkfifo "$QWATCH_FIFO" 2>/dev/null && exec 9<>"$QWATCH_FIFO" 2>/dev/null; then
        # Predicate: "the most recent queue-operation record is an
        # `enqueue`" == the host has input queued that it has not drained.
        # Exit 0 => abort; anything else or any error => do NOT abort.
        cat > "$QWATCH_PY" << 'QPYEOF'
import json, os, sys

path = sys.argv[1]
try:
    size = os.path.getsize(path)
except OSError:
    sys.exit(1)  # fail-open: cannot stat -> never abort

offset = max(0, size - 2_000_000)
last_op = None
try:
    with open(path, encoding='utf-8', errors='replace') as f:
        if offset:
            f.seek(offset)
            f.readline()  # discard partial line at the seek boundary
        for line in f:
            try:
                ev = json.loads(line)
            except Exception:
                continue
            if not isinstance(ev, dict):
                continue
            # Match the STRUCTURED record, never a substring. An `enqueue`
            # record embeds the full agent result in `content`, so a
            # transcript that merely DISCUSSES this predicate contains the
            # literal bytes "operation":"enqueue" inside an unrelated
            # record. A grep would abort spuriously and silently kill
            # listen mode -- the exact failure that got the raw
            # byte-growth approach rejected.
            if ev.get("type") != "queue-operation":
                continue
            last_op = ev.get("operation")
except OSError:
    sys.exit(1)  # fail-open

# Why "last op is an enqueue" and NOT "enqueues > dequeues":
#
# The queue vocabulary observed across 96 real transcripts is
# {enqueue: 710, dequeue: 434, remove: 269, popAll: 6} -- `remove` (a
# queued message deleted before it ran) and `popAll` also drain the queue.
# A naive enqueue/dequeue counter never sees them, so its count LATCHES
# permanently positive after the first `remove`, aborting every
# subsequent Stop hook and silently disabling listen mode for the rest of
# the session. Simulated over those transcripts, the counting predicate
# would abort at 79.8% of positions in the worst session; this last-op
# predicate, 46.1%.
#
# The last-op rule is self-clearing: ANY drain op (dequeue/remove/popAll)
# clears it, whatever the counts say, so it cannot latch on a vocabulary
# we have not seen. It also needs only the final record, so the 2 MB tail
# bound can never truncate it into a false positive.
#
# It covers both cases that matter: an enqueue that lands while we are
# blocked (the reported bug), and an enqueue already outstanding when the
# hook starts (a background agent that finished mid-turn -- a byte
# baseline captured at hook start would never see it).
#
# Its one miss is enqueue,enqueue,dequeue (one item still queued, last op
# is a drain). That fails toward NOT aborting -- i.e. toward today's
# behavior -- and the next enqueue fires it. Never abort on uncertainty.
sys.exit(0 if last_op == "enqueue" else 1)
QPYEOF
        # Watcher subshell inherits fd 9. Poll from t=0. On the predicate
        # firing, write one byte and exit. Any python error => loop without
        # writing (fail-open). Reaped when `famp await` returns regardless.
        (
            while : ; do
                if python3 "$QWATCH_PY" "$TRANSCRIPT" 2>/dev/null ; then
                    printf 'x' >&9 2>/dev/null || true
                    break
                fi
                sleep "${FAMP_QWATCH_INTERVAL:-2}"
            done
        ) &
        QWATCH_PID=$!
        ABORT_FD_READY=1
        log "cancellation watcher armed (pid=$QWATCH_PID)"
    else
        log "abort fifo setup failed; running plain await (fail-open)"
    fi
else
    log "abort fifo mktemp failed; running plain await (fail-open)"
fi

# Run `famp await`, arming --abort-on-fd 9 only when the watcher is up.
run_await() {
    if [ -n "$ABORT_FD_READY" ]; then
        "$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h --abort-on-fd 9
    else
        "$FAMP_BIN" await --as "$ACTIVE_IDENTITY" --timeout 23h
    fi
}

# --- Block on inbox ---------------------------------------------------
ERR_FILE="$(mktemp "${TMPDIR:-/tmp}/famp-await-err.XXXXXX")" || ERR_FILE=""
if [ -n "$ERR_FILE" ]; then
    MSG=$(run_await 2>"$ERR_FILE")
    STATUS=$?
    ERR=$(cat "$ERR_FILE" 2>/dev/null || true)
    rm -f "$ERR_FILE"
else
    MSG=$(run_await 2>&1)
    STATUS=$?
    ERR="$MSG"
fi
log "await returned status=$STATUS msg_bytes=${#MSG}"
[ -n "$ERR" ] && log "stderr: $ERR"

# --- Reap the cancellation watcher and release fd 9 -------------------
if [ -n "$QWATCH_PID" ]; then
    kill "$QWATCH_PID" 2>/dev/null || true
    wait "$QWATCH_PID" 2>/dev/null || true
fi
if [ -n "$ABORT_FD_READY" ]; then
    exec 9>&- 2>/dev/null || true
fi
if [ -n "$QWATCH_DIR" ]; then
    rm -rf "$QWATCH_DIR" 2>/dev/null || true
fi

# --- Abort path (issue #21): host queue has pending input -------------
# `famp await` exited 3 => a queued host notification is waiting. Exit 0
# with NO block decision so the turn ends and the host drains its queue.
# The `{"aborted":true}` sentinel on stdout is NOT forwarded to the host.
if [ "$STATUS" -eq 3 ]; then
    log "aborted: host queue has pending input; fail-open exit 0 so host drains"
    exit 0
fi

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

# --- Extract count + latest sender + mailbox kind/name for notification ---
# `famp await` now prints a single wrapper JSON object:
#   {"mailbox": {"kind": "channel"/"agent", "name": "..."}, "envelopes": [...]}
# Fallback branch handles legacy raw-envelope lines for backward compat.
META="$(python3 -c '
import json, sys
count = 0
sender = "unknown"
mailbox_kind = "agent"
mailbox_name = ""
for line in sys.argv[1].splitlines():
    line = line.strip()
    if not line:
        continue
    try:
        env = json.loads(line)
    except Exception:
        continue
    if env.get("timeout") is True:
        continue
    if isinstance(env.get("envelopes"), list):
        mb = env.get("mailbox") or {}
        if isinstance(mb, dict):
            mailbox_kind = mb.get("kind", "agent")
            mailbox_name = mb.get("name", "")
        for item in env["envelopes"]:
            if isinstance(item, dict):
                count += 1
                sender = item.get("from", item.get("sender", "unknown"))
        continue
    # Fallback: raw envelope line (backward compat)
    count += 1
    sender = env.get("from", env.get("sender", "unknown"))
    mailbox_kind = "agent"
    mailbox_name = ""
print(f"{count}|{sender}|{mailbox_kind}|{mailbox_name}")
' "$MSG" 2>/dev/null || printf '1|unknown|agent|\n')"
COUNT="${META%%|*}"
_REST="${META#*|}"
SENDER="${_REST%%|*}"
_REST2="${_REST#*|}"
MAILBOX_KIND="${_REST2%%|*}"
MAILBOX_NAME="${_REST2#*|}"
case "$COUNT" in
    ''|*[!0-9]*) COUNT=1 ;;
esac
if [ "$COUNT" -lt 1 ]; then
    log "await timeout payload; clean stop"
    exit 0
fi

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

if [ "$MAILBOX_KIND" = "channel" ]; then
    CHAN="$MAILBOX_NAME"
    case "$CHAN" in '#'*) ;; *) CHAN="#${CHAN}" ;; esac
    if [ "$COUNT" -gt 1 ]; then
        REASON="[FAMP listen mode] ${COUNT} new FAMP messages in channel ${CHAN}, latest from ${SENDER}. Call famp_channel_log({channel: '${CHAN}'}) to read them."
    else
        REASON="[FAMP listen mode] New FAMP message in channel ${CHAN} from ${SENDER}. Call famp_channel_log({channel: '${CHAN}'}) to read it."
    fi
else
    if [ "$COUNT" -gt 1 ]; then
        REASON="[FAMP listen mode] ${COUNT} new FAMP messages, latest from ${SENDER}. Call famp_inbox to read them."
    else
        REASON="[FAMP listen mode] New FAMP message from ${SENDER}. Call famp_inbox to read it."
    fi
fi
OUT=$(jq -n --arg r "$REASON" '{decision: "block", reason: $r}')
log "emitting block decision (${#OUT} bytes); count=$COUNT sender=$SENDER"
printf '%s\n' "$OUT"
