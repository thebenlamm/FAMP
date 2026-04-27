---
description: Truncate local FAMP inbox files (with confirmation). Pass --all, --agent <name>, --dry-run, --yes.
---

Run the famp-local clear subcommand with the user's arguments:

```bash
bash scripts/famp-local clear $ARGUMENTS
```

`$ARGUMENTS` forwards any flags the user typed after `/famp-clear`, e.g.:

- `/famp-clear --dry-run` — preview what would be cleared
- `/famp-clear --agent alice` — clear only alice's inbox
- `/famp-clear --all --yes` — clear local + federation inboxes without prompting

See `bash scripts/famp-local help` for the full flag list. Inboxes are truncated
in place (`: > inbox.jsonl`), not deleted, so any running `famp listen` keeps
its file descriptor valid.
