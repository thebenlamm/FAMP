---
name: famp-doctor
description: Diagnose FAMP broker health, registered sessions, and mailbox state. Use when agents aren't communicating, messages seem stuck, or you want a quick snapshot of who's alive and what's unread.
---

Run these three commands and report findings:

```bash
famp inspect broker
famp inspect identities
famp sessions
```

Then diagnose:

**Broker health** (`famp inspect broker`):
- `HEALTHY` → proceed
- `DOWN_CLEAN` / `STALE_SOCKET` / `ORPHAN_HOLDER` → broker is dead or stale; next FAMP command will auto-respawn it

**Sessions** (`famp sessions`):
- Lists live registered processes with their joined channels
- Cross-reference against `famp inspect identities` — if a name appears in identities but not sessions, the process died without deregistering (stale mailbox, not a live agent)

**Identities** (`famp inspect identities`):
- `LISTEN` column: `true` = auto-wakes on message; `false` = must be prompted manually to call `famp_inbox`
- `UNREAD` vs `TOTAL`: UNREAD shows messages past the agent's last-acked cursor. If UNREAD == TOTAL for an active agent, the cursor file may be at 0 — check `~/.famp/mailboxes/.<name>.cursor`
- `LAST_RECEIVED`: if stale by hours and agent shows UNREAD > 0 with `listen: false`, the agent needs a manual nudge to call `famp_inbox`

**Common issues and fixes**:

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| Agent not responding | `listen: false` + no nudge | Prompt agent to call `famp_inbox` |
| UNREAD == TOTAL for active agent | Cursor bug or fresh register | Check `cat ~/.famp/mailboxes/.<name>.cursor`; if 0, agent hasn't acked anything yet |
| Agent can't find a specific message | Stale `since` offset | Tell agent to call `famp_inbox` with no `since` (reads from 0) |
| No sessions showing | Broker idle-exited | Normal — broker auto-restarts on next FAMP command |
| All messages unread after broker restart | Register drain replays from 0 | Expected v0.9 behavior; agents re-drain full mailbox on re-register |

**To read an agent's unread messages directly** (without going through the agent):
```bash
python3 -c "
import json
with open('/Users/benlamm/.famp/mailboxes/<name>.jsonl') as f:
    lines = [l for l in f.read().split('\n') if l.strip()]
cursor = int(open('/Users/benlamm/.famp/mailboxes/.<name>.cursor').read().strip() or 0)
offset = 0
for line in lines:
    msg = json.loads(line)
    status = 'UNREAD' if offset >= cursor else 'read  '
    details = msg.get('body',{}).get('details',{})
    print(f'{status} [{offset:6d}] from={msg.get(\"from\",\"\").split(\"/\")[-1]:12} ts={msg.get(\"ts\",\"\")[11:19]} body={str(details.get(\"body\",\"\"))[:60]}')
    offset += len(line.encode()) + 1
"
```
