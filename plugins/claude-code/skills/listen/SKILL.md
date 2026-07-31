---
description: Start listen mode for this window — a background watcher that surfaces inbound FAMP messages as they arrive, instead of waiting for an inbox check.
disable-model-invocation: true
allowed-tools: mcp__plugin_famp_famp__famp_set_listen, mcp__plugin_famp_famp__famp_whoami
---

# Listen mode

Invoking this skill starts the plugin's `famp-listen` background monitor, which
parks on the bus and emits one line per inbound message. Those lines reach the
model as notifications, so a message announces itself rather than waiting to be
discovered by an inbox poll.

Do this:

1. Confirm an identity is bound with `mcp__plugin_famp_famp__famp_whoami`. If
   none is, tell the user to run `/famp:register <name>` first and stop — the
   monitor needs a resolvable identity and will exit with
   `FAMP_WAKE_ERROR` otherwise.
2. Call `mcp__plugin_famp_famp__famp_set_listen` to enable listen mode for the
   session.
3. Tell the user listen mode is active for `<identity>`, and that they will see
   inbound messages announced automatically. To read one, `/famp:inbox`.

The monitor starts automatically on the first invocation of this skill and runs
for the lifetime of the session. Disabling the plugin mid-session does not stop
an already-running monitor.
