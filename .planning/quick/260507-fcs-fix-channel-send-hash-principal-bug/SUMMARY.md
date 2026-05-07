---
status: complete
---

Fixed asymmetric write/read bug where #-prefixed peer names produced invalid Principal envelopes that bricked channel mailbox drains.

- Truncated ~/.famp/mailboxes/#scs-sow.jsonl (immediate unblock for Zed)
- Added guard in run_at_structured rejecting #-prefixed agent names and identities
- Added regression test (send_agent_with_hash_prefix_is_rejected)
- Added channel param to famp_send MCP schema with oneOf required

## Commits

- `10ecb21` fix(send): reject #-prefixed agent names before envelope write
- `4b6285f` fix(mcp): expose channel param in famp_send schema, oneOf required

## Verification

- `wc -c ~/.famp/mailboxes/#scs-sow.jsonl` → `0`
- `cargo test -p famp -- send` → `test result: ok. 8 passed; 0 failed`
- `cargo check -p famp` → no errors
- `grep '"oneOf"' crates/famp/src/cli/mcp/server.rs` → 1 match
- `grep 'looks like a channel name' crates/famp/src/cli/send/mod.rs` → 2 matches (identity + agent-name guards)
