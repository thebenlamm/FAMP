# Grok listen-wake review

The direction is right, but the current Grok path still depends too much on a model following prose. If the goal is to make onboarding simple and, above all, make it work, the wake lifecycle needs to be more mechanical.

What looks good:

- `famp listen-wake` is the right host-neutral wake primitive.
- The wake line is scrubbed and does not leak peer body bytes.
- Splitting wake notification from inbox retrieval is the right abstraction.
- Avoiding a long blocking Stop hook for Grok is sensible.

What is still fragile:

- `famp_register` only returns a `wake_hint.grok_monitor` string.
- The Grok skill tells the model to start a persistent monitor.
- Nothing here actually launches or supervises that monitor in code.
- `famp_set_listen(false)` only helps if a monitor is already running.
- Install/uninstall own the whole `~/.grok/skills/famp-listen/` tree, which is risky if that directory ever contains anything user-owned.
- The install path freezes a resolved `famp` command early, which can be brittle across shells, environments, or later binary moves.

The core problem is that the control plane is still prompt-driven:

1. register succeeds
2. the model notices the hint
3. the model starts the monitor
4. the monitor stays alive
5. listen-off is noticed
6. the monitor is killed

That is too many non-deterministic steps for the thing that is supposed to wake agents.

What I think is better:

- keep `famp listen-wake` as the single core primitive
- move monitor startup/supervision into a real adapter or host-side mechanism
- treat the skill as documentation, not the mechanism that makes wake work
- make uninstall remove only files FAMP clearly owns

Acceptance criteria I would want:

- install creates one clear MCP entry and one clear wake adapter
- a fresh Grok session can wake without manual re-arming by the model
- listen-off stops future wake behavior predictably
- uninstall does not delete user-owned content
- the flow still works if the skill text is ignored

Bottom line: the current design is good at defining the wake signal, but not yet good enough at guaranteeing the wake lifecycle. The next step should be to move monitor management out of the skill and into executable host-side code.
