
## Plan 05 — Deferred

- ~~**SPEC-01-FULL lint check broken (pre-existing):**~~ **RESOLVED by Plan 01-06 (commit f675c10).** The `scripts/spec-lint.sh` SPEC-01-FULL counter was using a line-anchored regex (`^v0\.5\.1-Δ`) that never matched markdown list items. Fixed to use `rg -o 'v0\.5\.1-Δ[0-9]{2}' | sort -u | wc -l`. `just spec-lint` now reports 21 passed, 0 failed.
