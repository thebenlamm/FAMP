# FAMP beta feedback: MCP listen mode does not automatically wake Codex

**Environment:** Codex MCP session, FAMP 0.5.2  
**Observed:** 2026-07-21

## Summary

I registered a Codex MCP session as `codex-0721` using
`famp_register(identity="codex-0721", listen=true)`. FAMP confirmed registration
and later confirmed `listen_mode: true`. A peer, `grok-0721`, successfully
delivered terminal replies while the Codex turn was idle, but those replies did
not wake the session automatically.

The broker appears to have delivered and queued the messages correctly. The
failure appears to be at the Codex client/Stop-hook integration boundary: no
FAMP waiter was parked after the turn ended.

## Steps to reproduce

1. Register a Codex MCP session using `listen=true`.
2. Send a task to another FAMP peer.
3. End the Codex turn without explicitly calling `famp_await`.
4. Have the peer reply after the turn ends.
5. Observe that Codex does not resume automatically.
6. Start another turn manually and call `famp_inbox`.
7. Observe that the peer reply was successfully delivered and queued.
8. Call `famp_inspect_waiters`; it reports `{"rows":[]}` despite listen mode
   being enabled.
9. Call `famp_set_listen({listen:true})`; it confirms
   `{"listen_mode":true}`, but no explanation is given for the missing waiter.

## Expected behavior

When an MCP session has `listen_mode=true`, ending the turn should park a FAMP
waiter through the documented Stop hook. A new peer message should wake the
Codex session automatically.

## Actual behavior

The message is written to the mailbox, but the Codex session has no active
waiter and is not awakened. It processes the message only after unrelated user
input starts another turn and the inbox is checked manually.

## Evidence

- Registration returned `active: "codex-0721"` with listening requested.
- Peer sends reported successful delivery.
- `famp_inbox` later returned the queued terminal replies.
- `famp_whoami` confirmed the active identity.
- `famp_inspect_waiters` returned no parked clients.
- `famp_set_listen({listen:true})` confirmed that listen mode was enabled.

## Impact

Agent-to-agent workflows that depend on “ping me when finished” silently stall.
The sender believes delivery succeeded, while the recipient does not resume.
Users must manually wake the agent and request an inbox check, undermining
FAMP's advertised asynchronous coordination.

## Likely fault boundary

Broker delivery appears healthy. The likely failure is in the Codex MCP
Stop-hook integration: listen mode is stored, but the host does not invoke or
maintain the post-turn `famp_await` waiter.

## Suggested improvements

- Verify that Codex hosts support the documented Stop hook before advertising
  automatic wake behavior.
- Expose `listen_mode` and waiter status together through `famp_whoami`.
- Return a warning when `listen_mode=true` but no waiter is parked after a turn.
- Document whether automatic wake currently works only in Claude-based clients.
- Provide a host-independent subscription or recurring-wait mechanism for
  Codex.
- Distinguish `listen_configured` from `actively_waiting` in diagnostics.
