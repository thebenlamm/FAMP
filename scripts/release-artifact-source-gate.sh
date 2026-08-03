#!/bin/sh
# scripts/release-artifact-source-gate.sh
#
# DIST-05: "only the tag-triggered release workflow produces release
# artifacts" as a mechanical, non-vacuous check, not merely an intention.
#
# Two assertions:
#
#   1. Sole producer — no workflow file other than .github/workflows/
#      release.yml may create/upload a GitHub Release asset. Comment lines
#      are stripped before matching (this repo has a recorded incident
#      where an unfiltered `grep -c` counted comment text as a violation —
#      see T-16-08), so a workflow's own explanatory header cannot self-
#      trip the gate.
#
#   2. Tag-gated trigger — release.yml's `on:` block must carry a `tags:`
#      key nested under `push:`. Note deliberately: release.yml also
#      legitimately carries a `pull_request:` trigger for its plan-only
#      dry-run job (no publishing happens on that path — see the `plan`
#      job's `publishing` output, gated on `!github.event.pull_request`).
#      A gate that demanded `push: tags:` be the *only* trigger would be
#      wrong, and would tempt a future editor to weaken it. What DIST-05
#      actually forbids is a *second producer* of artifacts, which
#      Assertion 1 covers.
#
# USAGE: scripts/release-artifact-source-gate.sh (no arguments)
# Exit 0 = both assertions hold. Exit 1 = at least one violation, each
# printed as an ::error::-prefixed line naming the offending file.

set -eu

SELF_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH='' cd -- "${SELF_DIR}/.." && pwd)
WORKFLOWS_DIR="${ROOT_DIR}/.github/workflows"
RELEASE_YML="${WORKFLOWS_DIR}/release.yml"

FAIL=0

# --- Assertion 1: sole producer ---------------------------------------
# Release-asset-creating mechanisms this gate watches for.
PRODUCER_PATTERN='gh release create|gh release upload|softprops/action-gh-release|actions/upload-release-asset|ncipollo/release-action'

for f in "${WORKFLOWS_DIR}"/*.yml "${WORKFLOWS_DIR}"/*.yaml; do
  [ -f "${f}" ] || continue
  base=$(basename "${f}")
  [ "${base}" = "release.yml" ] && continue
  # Strip comment-only lines (leading whitespace then `#`) before matching,
  # so a workflow's own header prose cannot self-trip this gate.
  stripped=$(grep -v '^[[:space:]]*#' "${f}" || true)
  if printf '%s\n' "${stripped}" | grep -qE "${PRODUCER_PATTERN}"; then
    echo "::error::${f} contains a release-asset-creating mechanism outside release.yml (DIST-05 violation)" >&2
    printf '%s\n' "${stripped}" | grep -nE "${PRODUCER_PATTERN}" >&2 || true
    FAIL=1
  fi
done

# --- Assertion 2: tag-gated trigger ------------------------------------
if [ ! -f "${RELEASE_YML}" ]; then
  echo "::error::${RELEASE_YML} not found" >&2
  FAIL=1
else
  ON_BLOCK=$(awk '/^on:/{flag=1} flag{print} /^jobs:/{exit}' "${RELEASE_YML}")
  PUSH_LINE=$(printf '%s\n' "${ON_BLOCK}" | grep -n '^[[:space:]]*push:' | head -1 | cut -d: -f1 || true)
  if [ -z "${PUSH_LINE}" ]; then
    echo "::error::${RELEASE_YML} — on: block has no push: trigger (DIST-05 violation)" >&2
    FAIL=1
  else
    AFTER_PUSH=$(printf '%s\n' "${ON_BLOCK}" | tail -n +"$((PUSH_LINE + 1))")
    TAGS_LINE=$(printf '%s\n' "${AFTER_PUSH}" | grep -n '^[[:space:]][[:space:]]*tags:' | head -1 || true)
    if [ -z "${TAGS_LINE}" ]; then
      echo "::error::${RELEASE_YML} — push: trigger has no nested tags: key (DIST-05 violation)" >&2
      FAIL=1
    fi
  fi
fi

if [ "${FAIL}" -eq 0 ]; then
  echo "OK - release-artifact-source-gate: release.yml is the sole tag-triggered producer (DIST-05)."
fi

exit "${FAIL}"
