# Grok listen-wake review

## Status (2026-07-22)

**Addressed by Stop path.** `famp install-grok` now installs a long Stop
hook (`famp-await.sh`, timeout 86400) — same foolproof mechanism as
Claude/Codex. User says "register with famp" → `famp_register` only; no
monitor memory required.

### Residual notes (historical)

Earlier design used `famp listen-wake` + model-started monitor. That path
depended on the model following skill prose. Stop `decision:block` is now
primary; monitor/`listen-wake --follow` is optional fallback only.

Grok host limit remains: **8 Stop continuations per turn**. After the cap
the turn ends; the next user prompt re-arms the hook.

### What still holds

- `famp listen-wake` remains the host-neutral wake primitive for non-Stop
  hosts.
- Wake lines stay scrubbed (no peer body).
- Uninstall-grok removes only `~/.grok/` artifacts; leaves `~/.claude/`
  alone so Claude Code's primary famp-await is not torn down.
