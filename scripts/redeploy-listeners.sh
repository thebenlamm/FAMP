#!/usr/bin/env bash
# redeploy-listeners.sh — safe rebuild + restart of all FAMP listener daemons.
#
# WHY THIS EXISTS:
#   A series of fixes shipped code changes that could not reach the live
#   listener daemons because there was no documented redeploy path.
#   The stale ~/.cargo/bin/famp silently masked every fix.  This script
#   is the missing operational piece.
#
# WHAT THIS SCRIPT DOES:
#   1. Refuses to rebuild from a dirty crates/famp/ tree (footgun guard).
#   2. Refuses to proceed if any task TOML is in a non-terminal state
#      (REQUESTED or COMMITTED), unless --force is passed.
#   3. Rebuilds ~/.cargo/bin/famp via `cargo install --path crates/famp`
#      (skipped with --no-rebuild).
#   4. Sends SIGTERM to each running daemon; escalates to SIGKILL after 8s.
#   5. Restarts each daemon with FAMP_HOME set; appends to daemon.log.
#   6. Polls daemon.log for a fresh "listening on https://" line (10s timeout).
#   7. Prints a per-agent success/failure summary table.
#
# WHAT THIS SCRIPT INTENTIONALLY DOES NOT DO:
#   - Does NOT modify scripts/famp-local or any daemon source code.
#   - Does NOT run from CI or any automated trigger (operator-only tool).
#   - Does NOT truncate daemon.log — appends only, preserving restart history.
#   - Does NOT hard-code agent names — discovers from ~/.famp-local/agents/*.
#
# SHELLCHECK SUPPRESSIONS (if any) are documented inline at each site.

set -euo pipefail

# ─── state layout (mirrors scripts/famp-local verbatim) ────────────────────

STATE_ROOT="${FAMP_LOCAL_ROOT:-$HOME/.famp-local}"
FAMP_BIN="${FAMP_BIN:-famp}"

home_for() { echo "$STATE_ROOT/agents/$1"; }
pid_for()  { echo "$STATE_ROOT/agents/$1/daemon.pid"; }
log_for()  { echo "$STATE_ROOT/agents/$1/daemon.log"; }

# ─── helpers (mirrors scripts/famp-local verbatim) ─────────────────────────

die()  { printf 'redeploy-listeners: %s\n' "$*" >&2; exit 1; }
note() { printf '  %s\n' "$*" >&2; }

# Return 0 if PID $1 is alive, 1 otherwise.
is_alive() {
  local pid="$1"
  [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

# ─── flag parsing ───────────────────────────────────────────────────────────

DRY_RUN=0
FORCE=0
NO_REBUILD=0

usage() {
  cat <<'EOF'
Usage: scripts/redeploy-listeners.sh [OPTIONS]

Rebuild the famp binary and safely restart all running listener daemons.

OPTIONS
  --dry-run, --check   Print what would happen; take no destructive action.
  --force              Skip the in-flight task safety check.
  --no-rebuild         Skip `cargo install`; just cycle daemons.
  -h, --help           Show this message and exit 0.

ENVIRONMENT
  FAMP_BIN             Path to famp binary (default: famp on PATH).
  FAMP_LOCAL_ROOT      State dir (default: ~/.famp-local).

SAFETY GUARDS (waived by flags as noted)
  - Refuses to run if crates/famp/ has uncommitted changes (unless --no-rebuild).
  - Refuses to run if any task TOML is in a non-terminal state (unless --force).
  - Never truncates daemon.log — appends only.
  - Never hard-codes agent names — discovers from ~/.famp-local/agents/*.

EXAMPLES
  scripts/redeploy-listeners.sh             # interactive: prompts before killing
  scripts/redeploy-listeners.sh --dry-run   # show plan, take no action
  scripts/redeploy-listeners.sh --force     # skip in-flight-task check
  scripts/redeploy-listeners.sh --no-rebuild # cycle daemons only, no cargo install
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run|--check) DRY_RUN=1; shift ;;
    --force)           FORCE=1;   shift ;;
    --no-rebuild)      NO_REBUILD=1; shift ;;
    -h|--help)         usage; exit 0 ;;
    *)                 die "unknown flag: $1 (try --help)" ;;
  esac
done

# ─── locate repo root ───────────────────────────────────────────────────────

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" \
  || die "not inside a git repository; cannot locate crates/famp/"

# ─── step 1: repo-clean guard (skip if --no-rebuild) ───────────────────────

if [ "$NO_REBUILD" = 0 ]; then
  dirty="$(git -C "$REPO_ROOT" status --porcelain crates/famp/ 2>/dev/null || true)"
  if [ -n "$dirty" ]; then
    die "crates/famp/ has uncommitted changes — commit or stash first, OR pass --no-rebuild to skip the rebuild entirely.
Dirty files:
$dirty"
  fi
fi

# ─── step 2: in-flight task safety guard (skip if --force) ─────────────────

if [ "$FORCE" = 0 ]; then
  inflight_list=""
  # Use nullglob-equivalent: iterate only if files exist.
  # shellcheck disable=SC2012  # ls not needed; we use find so empty matches cleanly
  while IFS= read -r toml; do
    [ -f "$toml" ] || continue
    state_val="$(awk -F'"' '/^state[[:space:]]*=/ { print $2 }' "$toml")"
    case "$state_val" in
      COMPLETED|FAILED|CANCELLED) continue ;;
      "")
        note "warning: could not read state from $toml — treating as non-terminal"
        ;;
    esac
    # Non-terminal or unreadable — record it.
    agent_dir="$(dirname "$(dirname "$toml")")"
    agent_name="$(basename "$agent_dir")"
    task_id="$(awk -F'"' '/^task_id[[:space:]]*=/ { print $2 }' "$toml")"
    inflight_list="${inflight_list}  agent=${agent_name}  task=${task_id}  state=${state_val}  file=${toml}
"
  done < <(find "$STATE_ROOT/agents" -path "*/tasks/*.toml" 2>/dev/null || true)

  if [ -n "$inflight_list" ]; then
    printf 'redeploy-listeners: in-flight tasks would be interrupted:\n%s\n' "$inflight_list" >&2
    die "pass --force to override (you own the consequences for in-flight tasks)"
  fi
fi

# ─── step 3: discover daemons ───────────────────────────────────────────────

# Parallel arrays: names / pids / homes / logs for daemons we will cycle.
AGENT_NAMES=()
AGENT_PIDS=()
AGENT_HOMES=()
AGENT_LOGS=()

# Iterate PID files; skip stale ones.
while IFS= read -r pidf; do
  [ -f "$pidf" ] || continue
  # Extract agent name: …/agents/<name>/daemon.pid
  agent_name="$(basename "$(dirname "$pidf")")"
  pid="$(cat "$pidf" 2>/dev/null || true)"
  if [ -z "$pid" ]; then
    note "warning: empty PID file for $agent_name — skipping"
    continue
  fi
  if ! is_alive "$pid"; then
    note "stale PID file for $agent_name (pid=$pid, process dead) — cleaning up"
    if [ "$DRY_RUN" = 0 ]; then
      rm -f "$pidf"
    else
      note "[dry-run] would remove stale PID file: $pidf"
    fi
    continue
  fi
  home="$(home_for "$agent_name")"
  logf="$(log_for "$agent_name")"
  AGENT_NAMES+=("$agent_name")
  AGENT_PIDS+=("$pid")
  AGENT_HOMES+=("$home")
  AGENT_LOGS+=("$logf")
done < <(find "$STATE_ROOT/agents" -name "daemon.pid" 2>/dev/null | sort || true)

if [ "${#AGENT_NAMES[@]}" -eq 0 ]; then
  note "no running listeners found under $STATE_ROOT/agents/ — nothing to cycle"
  exit 0
fi

# ─── step 4: print plan (and exit here if --dry-run) ───────────────────────

echo "" >&2
echo "redeploy-listeners plan:" >&2
echo "  rebuild:  $([ "$NO_REBUILD" = 0 ] && echo "YES (cargo install --path crates/famp)" || echo "SKIPPED (--no-rebuild)")" >&2
echo "  agents:   ${#AGENT_NAMES[@]}" >&2
for i in "${!AGENT_NAMES[@]}"; do
  printf '    %-16s pid=%-8s log=%s\n' \
    "${AGENT_NAMES[$i]}" "${AGENT_PIDS[$i]}" "${AGENT_LOGS[$i]}" >&2
done
echo "" >&2

if [ "$DRY_RUN" = 1 ]; then
  echo "redeploy-listeners: --dry-run mode — no action taken." >&2
  exit 0
fi

# Interactive prompt: confirm before destructive steps (skip if --force or stdin is not a tty).
if [ "$FORCE" = 0 ] && [ -t 0 ]; then
  printf 'Proceed with stop → rebuild → restart? [y/N] ' >&2
  read -r answer
  case "$answer" in
    y|Y|yes|YES) ;;
    *) die "aborted by operator" ;;
  esac
fi

# ─── step 5: rebuild (skip if --no-rebuild) ─────────────────────────────────

if [ "$NO_REBUILD" = 0 ]; then
  echo "redeploy-listeners: rebuilding famp binary..." >&2
  if ! cargo install --path "$REPO_ROOT/crates/famp" >&2; then
    die "rebuild failed — daemons left untouched"
  fi
  echo "redeploy-listeners: rebuild complete." >&2
fi

# ─── step 6: stop daemons (SIGTERM + poll + SIGKILL fallback) ───────────────

# Poll interval: 100ms.  Max tries before escalating to SIGKILL: 80 = 8s.
STOP_MAX_TRIES=80
STOP_SLEEP_INTERVAL=0.1

# Per-agent stop results: "clean", "killed", "failed"
AGENT_STOP_RESULTS=()

for i in "${!AGENT_NAMES[@]}"; do
  name="${AGENT_NAMES[$i]}"
  pid="${AGENT_PIDS[$i]}"
  pidf="$(pid_for "$name")"

  if ! is_alive "$pid"; then
    note "$name: process already gone before stop (pid=$pid)"
    AGENT_STOP_RESULTS+=("already-gone")
    rm -f "$pidf"
    continue
  fi

  note "$name: sending SIGTERM (pid=$pid)"
  kill -TERM "$pid" 2>/dev/null || true

  tries=0
  while is_alive "$pid" && [ "$tries" -lt "$STOP_MAX_TRIES" ]; do
    sleep "$STOP_SLEEP_INTERVAL"
    tries=$((tries + 1))
  done

  if is_alive "$pid"; then
    printf 'redeploy-listeners: warning: %s did not exit within 8s after SIGTERM — escalating to SIGKILL\n' \
      "$name" >&2
    kill -KILL "$pid" 2>/dev/null || true
    # Give the kernel a moment to reap.
    sleep 0.2
    if is_alive "$pid"; then
      note "$name: SIGKILL also failed — process $pid may be unkillable"
      AGENT_STOP_RESULTS+=("failed")
      continue
    fi
    AGENT_STOP_RESULTS+=("killed")
  else
    AGENT_STOP_RESULTS+=("clean")
  fi

  rm -f "$pidf"
  note "$name: stopped (result=${AGENT_STOP_RESULTS[$i]})"
done

# ─── step 7: restart daemons ────────────────────────────────────────────────

# Per-agent restart results: "success", "timeout", "process-died"
AGENT_RESTART_RESULTS=()
AGENT_NEW_PIDS=()

RESTART_MAX_TRIES=100   # 100 × 100ms = 10s
RESTART_SLEEP_INTERVAL=0.1

for i in "${!AGENT_NAMES[@]}"; do
  name="${AGENT_NAMES[$i]}"
  home="${AGENT_HOMES[$i]}"
  logf="${AGENT_LOGS[$i]}"

  if [ "${AGENT_STOP_RESULTS[$i]}" = "failed" ]; then
    note "$name: skipping restart — stop failed"
    AGENT_RESTART_RESULTS+=("skipped-stop-failed")
    AGENT_NEW_PIDS+=("—")
    continue
  fi

  # Record the number of lines currently in the log so we can scope
  # the "listening on https://" search to lines appearing AFTER restart.
  log_lines_before=0
  if [ -f "$logf" ]; then
    log_lines_before="$(wc -l < "$logf" 2>/dev/null || echo 0)"
  fi
  # Capture restart epoch for an alternative time-based scope (belt + braces).
  restart_epoch="$(date +%s)"

  note "$name: starting daemon (FAMP_HOME=$home, log=$logf)"

  # APPEND redirect (>>) — required by task brief; preserves restart history.
  # This intentionally differs from famp-local's truncating (>) redirect.
  FAMP_HOME="$home" nohup "$FAMP_BIN" listen >>"$logf" 2>&1 &
  new_pid=$!

  pidf="$(pid_for "$name")"
  echo "$new_pid" > "$pidf"
  AGENT_NEW_PIDS+=("$new_pid")

  # Wait until the process is confirmed alive before polling the log.
  sleep 0.1
  if ! is_alive "$new_pid"; then
    note "$name: process died immediately after launch — check $logf"
    AGENT_RESTART_RESULTS+=("process-died")
    continue
  fi

  # Poll log for a fresh "listening on https://" beacon.
  beacon_found=0
  tries=0
  next_line=$((log_lines_before + 1))
  while [ "$tries" -lt "$RESTART_MAX_TRIES" ]; do
    if [ -f "$logf" ]; then
      # Read lines that appeared after the restart (tail from next_line onward).
      # The daemon writes "listening on https://<addr>" to stderr, which nohup
      # redirects into logf.
      if tail -n +"${next_line}" "$logf" 2>/dev/null | grep -q "listening on https://"; then
        beacon_found=1
        break
      fi
    fi
    # Belt-and-braces: if process has already exited, bail early.
    if ! is_alive "$new_pid"; then
      note "$name: process exited before beacon — check $logf"
      AGENT_RESTART_RESULTS+=("process-died")
      beacon_found=-1
      break
    fi
    sleep "$RESTART_SLEEP_INTERVAL"
    tries=$((tries + 1))
  done

  if [ "$beacon_found" = 1 ]; then
    note "$name: listening (pid=$new_pid)"
    AGENT_RESTART_RESULTS+=("success")
  elif [ "$beacon_found" = 0 ]; then
    note "$name: timed out waiting for beacon — check $logf"
    AGENT_RESTART_RESULTS+=("timeout")
  fi
  # beacon_found=-1 already pushed result above.

  # Suppress unused variable warning; restart_epoch used as documentation
  # of the time boundary — the tail-from-line approach is authoritative.
  : "$restart_epoch"
done

# ─── step 9: final summary ───────────────────────────────────────────────────

echo "" >&2
echo "─────────────────────────────────────────────────────────────────" >&2
printf '%-16s  %-12s  %-14s  %-8s  %s\n' \
  "AGENT" "STOP" "RESTART" "PID" "LOG" >&2
echo "─────────────────────────────────────────────────────────────────" >&2
for i in "${!AGENT_NAMES[@]}"; do
  printf '%-16s  %-12s  %-14s  %-8s  %s\n' \
    "${AGENT_NAMES[$i]}" \
    "${AGENT_STOP_RESULTS[$i]}" \
    "${AGENT_RESTART_RESULTS[$i]}" \
    "${AGENT_NEW_PIDS[$i]}" \
    "${AGENT_LOGS[$i]}" >&2
done
echo "─────────────────────────────────────────────────────────────────" >&2

# Determine exit code.
any_failure=0
for i in "${!AGENT_NAMES[@]}"; do
  case "${AGENT_RESTART_RESULTS[$i]}" in
    success) ;;
    *)
      any_failure=1
      printf 'redeploy-listeners: FAIL  %s — stop=%s restart=%s\n' \
        "${AGENT_NAMES[$i]}" "${AGENT_STOP_RESULTS[$i]}" "${AGENT_RESTART_RESULTS[$i]}" >&2
      printf '  inspect: tail -50 %s\n' "${AGENT_LOGS[$i]}" >&2
      printf '  liveness: kill -0 %s\n' "${AGENT_NEW_PIDS[$i]}" >&2
      ;;
  esac
done

if [ "$any_failure" = 0 ]; then
  echo "redeploy-listeners: all ${#AGENT_NAMES[@]} agent(s) cycled cleanly." >&2
  exit 0
else
  exit 1
fi
