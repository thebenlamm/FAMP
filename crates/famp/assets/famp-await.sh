#!/usr/bin/env bash
# FAMP inbound listen-mode Stop hook (Claude Code + Codex).
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

# --- Read hook metadata from stdin BEFORE redirecting stdin --------------
STDIN_JSON="$(cat 2>/dev/null || true)"
TRANSCRIPT="$(printf '%s' "$STDIN_JSON" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("transcript_path",""))' \
    2>/dev/null || true)"
SESSION_ID="$(printf '%s' "$STDIN_JSON" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("session_id",""))' \
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

# --- Codex rollout fallback --------------------------------------------
# Some Codex Stop-hook payloads omit transcript_path. Resolve the rollout
# path from Codex's state DB by session_id so listen mode still arms.
if { [ -z "$TRANSCRIPT" ] || [ ! -f "$TRANSCRIPT" ]; } && [ -n "$SESSION_ID" ]; then
    RESOLVED_TRANSCRIPT="$(python3 -c '
import glob, os, sys, urllib.parse

try:
    import sqlite3
except Exception:
    sqlite3 = None

session_id = sys.argv[1]
home = os.environ.get("HOME") or ""
codex_home = os.environ.get("CODEX_HOME") or (os.path.join(home, ".codex") if home else "")
sqlite_home = os.environ.get("CODEX_SQLITE_HOME") or codex_home
session_root = os.path.join(codex_home, "sessions") if codex_home else ""

db_candidates = []
if sqlite_home:
    db_candidates.append(os.path.join(sqlite_home, "state_5.sqlite"))
if codex_home and codex_home != sqlite_home:
    db_candidates.append(os.path.join(codex_home, "state_5.sqlite"))

def allowed_rollout_path(path):
    if not path or not os.path.isfile(path) or not session_root:
        return False
    try:
        real_path = os.path.realpath(path)
        real_root = os.path.realpath(session_root)
        return os.path.commonpath([real_path, real_root]) == real_root
    except Exception:
        return False

if sqlite3 is not None:
    for db in db_candidates:
        if not os.path.isfile(db):
            continue
        try:
            uri = "file:" + urllib.parse.quote(os.path.abspath(db)) + "?mode=ro"
            con = sqlite3.connect(uri, uri=True, timeout=0.2)
            con.execute("pragma query_only = ON")
            row = con.execute("select rollout_path from threads where id = ?", (session_id,)).fetchone()
            con.close()
        except Exception:
            row = None
        if row and allowed_rollout_path(row[0]):
            print(row[0])
            sys.exit(0)

if codex_home:
    pattern = os.path.join(codex_home, "sessions", "**", f"rollout-*{session_id}.jsonl")
    matches = [p for p in glob.glob(pattern, recursive=True) if os.path.isfile(p)]
    if matches:
        matches.sort(key=lambda p: os.path.getmtime(p), reverse=True)
        print(matches[0])
' "$SESSION_ID" 2>/dev/null || true)"
    if [ -n "$RESOLVED_TRANSCRIPT" ]; then
        TRANSCRIPT="$RESOLVED_TRANSCRIPT"
        log "resolved transcript from Codex session_id=$SESSION_ID"
    fi
fi

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
actions = []   # (line_pos, tool_use_id, kind, identity, listen)
results = {}   # tool_use_id -> ok (bool)

def parse_args(raw):
    if isinstance(raw, dict):
        return raw
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
        except Exception:
            return {}
        return parsed if isinstance(parsed, dict) else {}
    return {}

def function_output_success(payload):
    output = payload.get("output", "")
    if not isinstance(output, str):
        return True
    lowered = output.lower().replace(" ", "")
    return "\"iserror\":true" not in lowered and "\"is_error\":true" not in lowered

# Limit scan to last 2 MB to protect against huge transcripts.
MAX_BYTES = 2_000_000
fsize  = os.path.getsize(path)
offset = max(0, fsize - MAX_BYTES)

pos = 0
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
        # Claude Code transcript format.
        msg = ev.get("message") if isinstance(ev.get("message"), dict) else ev
        content = msg.get("content") or []
        if isinstance(content, str):
            content = []
        for block in content:
            if not isinstance(block, dict):
                continue
            t    = block.get("type", "")
            name = str(block.get("name", ""))
            if t == "tool_use":
                inp = parse_args(block.get("input"))
                uid = block.get("id", "")
                if name.endswith("famp_register"):
                    ident = inp.get("identity") or inp.get("name", "")
                    if ident and uid:
                        actions.append((pos, uid, "register", ident, inp.get("listen")))
                elif name.endswith("famp_set_listen") and uid:
                    actions.append((pos, uid, "set_listen", "", inp.get("listen")))
            elif t == "tool_result":
                uid = block.get("tool_use_id", "")
                # Strict boolean check: only JSON true (Python True) counts as error.
                # Avoids bool("false") == True for string-typed is_error values.
                results[uid] = block.get("is_error") is not True

        # Codex rollout JSONL format.
        payload = ev.get("payload") if isinstance(ev.get("payload"), dict) else {}
        if payload.get("type") == "function_call":
            tool = str(payload.get("name", ""))
            namespace = str(payload.get("namespace", ""))
            if namespace and namespace != "mcp__famp":
                tool = ""
            args = parse_args(payload.get("arguments"))
            uid = payload.get("call_id") or f"codex-fc:{pos}"
            if tool.endswith("famp_register"):
                ident = args.get("identity") or args.get("name", "")
                if ident:
                    actions.append((pos, uid, "register", ident, args.get("listen")))
            elif tool.endswith("famp_set_listen"):
                actions.append((pos, uid, "set_listen", "", args.get("listen")))
        elif payload.get("type") == "function_call_output":
            uid = payload.get("call_id", "")
            if uid and uid not in results:
                results[uid] = function_output_success(payload)

        if payload.get("type") == "mcp_tool_call_end":
            inv = payload.get("invocation") if isinstance(payload.get("invocation"), dict) else {}
            tool = str(inv.get("tool", ""))
            args = parse_args(inv.get("arguments"))
            result = payload.get("result") if isinstance(payload.get("result"), dict) else {}
            ok_payload = result.get("Ok") if isinstance(result.get("Ok"), dict) else None
            ok = ok_payload is not None and ok_payload.get("isError") is not True
            uid = payload.get("call_id") or f"codex:{pos}"
            if tool.endswith("famp_register"):
                ident = args.get("identity") or args.get("name", "")
                if ident:
                    actions.append((pos, uid, "register", ident, args.get("listen")))
                    results[uid] = ok
            elif tool.endswith("famp_set_listen"):
                actions.append((pos, uid, "set_listen", "", args.get("listen")))
                results[uid] = ok

# Replay successful control actions in transcript order. Listen defaults ON
# for register calls; only an explicit JSON false disables it. A later
# famp_set_listen(false) cancels listen mode without requiring re-register.
active = ""
last_identity = ""
for _, uid, kind, ident, listen in actions:
    if not results.get(uid, False):
        continue
    if kind == "register":
        last_identity = ident or last_identity
        active = ident if listen is not False else ""
    elif kind == "set_listen":
        if listen is False:
            active = ""
        elif last_identity:
            active = last_identity

print(active)
PYEOF

# --- Extract identity from transcript ---------------------------------
ACTIVE_IDENTITY="$(python3 "$PY_SCRIPT" "$TRANSCRIPT" 2>/dev/null || true)"
rm -f "$PY_SCRIPT"

FAMP_BIN="$(command -v famp 2>/dev/null || echo "$HOME/.cargo/bin/famp")"

if [ -z "$ACTIVE_IDENTITY" ]; then
    # --- Broker fallback (compaction resilience, 260721) ---------------
    # A long session whose transcript was compacted (Claude Code /compact)
    # can lose its famp_register marker out of the 2 MB scan tail above,
    # leaving no identity even though THIS window's MCP server still holds
    # a live listen=true registration. Recover it WITHOUT guessing from the
    # cwd (a cwd match would hijack an innocent, never-registered window
    # that merely shares the checkout — it would start awaiting on another
    # agent's identity). Instead correlate by process ancestry:
    #
    #   this hook  ── spawned by ──>  the Claude Code process  <── spawns ── `famp mcp`
    #
    # So THIS window's `famp mcp` server is the one whose parent pid appears
    # in this hook's ancestor chain. `famp sessions` maps that mcp pid to the
    # registered name; `inspect identities --json` confirms listen=true. We
    # adopt ONLY that identity — never one merely co-located by cwd. If the
    # process model doesn't cooperate (nothing resolves), we no-op exactly
    # as before: fail-open, and strictly never a hijack. Runs only when the
    # transcript yielded nothing, so normal Stops pay no cost.

    # 1. Ancestor pids of this hook (bounded walk; skip 0/1 so an mcp that
    #    got reparented to init can never false-match via pid 1).
    ANCESTORS=""
    _p="$$"
    _depth=0
    while [ "$_depth" -lt 6 ]; do
        _pp="$(ps -o ppid= -p "$_p" 2>/dev/null | tr -d ' ')"
        case "$_pp" in ''|0|1) break ;; esac
        ANCESTORS="$ANCESTORS $_pp"
        _p="$_pp"
        _depth=$((_depth + 1))
    done

    # 2. `famp mcp` pids whose parent is one of our ancestors == this
    #    window's mcp server(s). (The path "/.../bin/famp mcp" contains the
    #    literal substring "famp mcp".)
    SIBLING_MCP_PIDS=""
    if [ -n "$ANCESTORS" ]; then
        SIBLING_MCP_PIDS="$(ps -eo pid=,ppid=,args= 2>/dev/null \
            | awk -v anc="$ANCESTORS" '
                BEGIN { n = split(anc, a, " "); for (i = 1; i <= n; i++) if (a[i] != "") A[a[i]] = 1 }
                ($2 in A) && index($0, "famp mcp") { print $1 }' \
            2>/dev/null || true)"
    fi

    # 3. mcp pid -> registered name via `famp sessions` (adopt only a UNIQUE
    #    name; one mcp per window means this is normally exactly one).
    CANDIDATE=""
    if [ -n "$SIBLING_MCP_PIDS" ]; then
        CANDIDATE="$("$FAMP_BIN" sessions 2>/dev/null \
            | python3 -c 'import json, sys
pids = set(sys.argv[1].split())
names = []
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        row = json.loads(line)
    except Exception:
        continue
    if str(row.get("pid", "")) in pids and row.get("name"):
        names.append(row["name"])
uniq = sorted(set(names))
print(uniq[0] if len(uniq) == 1 else "")' \
            "$SIBLING_MCP_PIDS" 2>/dev/null || true)"
    fi

    # 4. Adopt only if that name is registered with listen_mode==true
    #    (a window that registered with listen:false opted out of auto-wake).
    if [ -n "$CANDIDATE" ]; then
        LISTEN_OK="$("$FAMP_BIN" inspect identities --json 2>/dev/null \
            | python3 -c 'import json, sys
name = sys.argv[1]
try:
    data = json.load(sys.stdin)
except Exception:
    sys.exit(0)
rows = data.get("rows", []) if isinstance(data, dict) else []
for r in rows:
    if isinstance(r, dict) and r.get("name") == name and r.get("listen_mode") is True:
        print("yes")
        break' \
            "$CANDIDATE" 2>/dev/null || true)"
        if [ "$LISTEN_OK" = "yes" ]; then
            ACTIVE_IDENTITY="$CANDIDATE"
            log "transcript had no register; pid-correlated fallback resolved identity=$ACTIVE_IDENTITY (sibling mcp pids:$SIBLING_MCP_PIDS)"
        else
            log "pid-correlated candidate '$CANDIDATE' is not listen=true; no-op"
        fi
    fi
fi

if [ -z "$ACTIVE_IDENTITY" ]; then
    log "no listen registration in transcript (and no pid-correlated listen identity); exiting no-op"
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
