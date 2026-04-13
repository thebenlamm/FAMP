
## Plan 05 — Deferred

- **SPEC-01-FULL lint check broken (pre-existing):** `just spec-lint` SPEC-01-FULL step reports "found 0" despite 22+ `v0.5.1-Δnn` entries present in file. Likely a shell/grep escaping bug in the Justfile recipe regarding the Δ (U+0394) character. Not in Plan 05 scope (all Δ entries Plan 05 introduced are present and matched by `rg 'v0\.5\.1-Δ24'`). Needs follow-up fix to Justfile `spec-lint` recipe.
