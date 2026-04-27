---
description: Check whether the current folder is set up for FAMP — shows identity, daemon, inbox status.
---

Check FAMP wiring status for the current folder (or a specified directory):

```bash
bash scripts/famp-local doctor $ARGUMENTS
```

`$ARGUMENTS` is optional — pass a directory to check that one instead of `$PWD`.

- `/famp-doctor` — check this folder
- `/famp-doctor ~/Workspace/Other` — check a specific folder

Output is read-only diagnostics: a clear ✓/⚠/✗ headline and details about
the identity, daemon, and inbox if wired.
