# Review: install-grok / listen-wake — 1 blocker, 2 fixes before merge

**Verdict:** design is right, ship it after these. Reused `upsert_codex_table`, kept peer body off the wake line, sender-scrubbing is solid — no notes there.

## Blocker — duplicate-wake race, fix before merge

Your skill says: *"if the monitor dies and listen is still true, restart it with the same command."* No pidfile, no lock, no kill-old-first.

Traced it in the broker: proxy connections (what `--as` uses) get `pid: None` by invariant (`famp-bus/src/broker/state.rs:10-20`) — the liveness sweep in `tick()` filters on `state.pid?`, so it **never reaps a dead proxy**. Cleanup is 100% socket-close detection. `waiting_clients_for_name`/`send_agent` (`handle.rs:424-462`) broadcast to **every** matching connected proxy. So: restart races old socket teardown → two live monitors on one identity → every future message double-wakes you, silently, forever (23h deadline).

**Fix:** pidfile under `~/.famp/listen-wake-<identity>.pid`; refuse to start if the pid in it is alive; on restart, kill it first. Not a broker fix — yours to make in `listen_wake.rs`.

## Fix — `--loop` dies silently, no recovery signal

`listen_wake.rs:742` propagates any bus error via `?` and the process exits. `famp daemon restart` = every Grok monitor dead with nothing telling the supervisor to relaunch. Add bounded retry-with-backoff inside `listen-wake` itself — don't push recovery onto the host's supervisor.

## Fix — untested code path

`hook_runner_await.rs` hardcodes `FAMP_DISABLE_PID_FALLBACK=1` in every test. The missing-`transcript_path` → PID-fallback path (the actual fix for the Codex bug you filed) has zero live coverage — only a grep-on-string assertion. Add one test that actually exercises it.

## Architecture — before host #4, not now

Your per-host Rust file is ~15 lines of real data (config path, JSON-vs-TOML, entry key) wrapped in 200 lines of boilerplate. Fine at N=3. Open an issue tagged `before-host-4`: collapse the MCP-merge into a config table so onboarding a host is a data row, not a new module + test suite. Leave the two wake mechanisms alone — that split is real, not accidental.
