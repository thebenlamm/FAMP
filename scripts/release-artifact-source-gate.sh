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

# --- Assertion 2: tag-gated trigger (indentation-based nesting) ----------
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
    # Verify indentation: push: must have tags: and must not have branches:
    # as direct children. Extract the push: line and determine its indentation
    # level, then check for tags: and branches: at the correct child level.
    AFTER_PUSH=$(printf '%s\n' "${ON_BLOCK}" | tail -n +"$((PUSH_LINE + 1))")

    # Count leading spaces in push: line
    PUSH_FULL=$(printf '%s\n' "${ON_BLOCK}" | sed -n "${PUSH_LINE}p")
    # Use sed to extract leading spaces, then count using bash string length
    LEADING_SPACES=$(printf '%s' "${PUSH_FULL}" | sed 's/^\( *\).*/\1/')
    PUSH_SPACES=${#LEADING_SPACES}  # bash string length

    # Child indentation is parent + 2 (standard 2-space indent in YAML)
    EXPECTED_CHILD_SPACES=$((PUSH_SPACES + 2))

    # DEBUG: uncomment to see indentation calculations
    # echo "DEBUG: PUSH_LINE=$PUSH_LINE, PUSH_FULL='$PUSH_FULL', PUSH_SPACES=$PUSH_SPACES, EXPECTED_CHILD_SPACES=$EXPECTED_CHILD_SPACES" >&2

    # Look for tags: and branches: as direct children of push:.
    #
    # Two traps this deliberately avoids, both of which shipped once and were
    # caught by constructing the inputs rather than by reading the code:
    #
    # 1. Do NOT anchor the key with /tags:$/. That only matches BLOCK form
    #    ("tags:" then an indented list). YAML flow form -- "tags: ['v*']",
    #    "branches: [main]" -- never matches an end-anchored pattern. The
    #    end-anchored version simultaneously PASSED a release.yml carrying
    #    `branches: [main]` next to `tags:` (publishing on every push to main,
    #    the exact violation this assertion exists to catch) and FAILED a
    #    legitimate `tags: ['v*']`. Match the KEY, and accept any value form.
    #
    # 2. Stop at the end of push:'s own mapping. Scanning to the end of the
    #    whole on: block means a SIBLING trigger's branches: filter (e.g.
    #    `pull_request:` with `branches:`) would be misattributed to push:.
    #    Any line that dedents back to push:'s own level or further has left
    #    the mapping.
    #
    # BSD awk's -v flag mis-parses some values, so the script is built by
    # substitution rather than passed with -v.
    AWK_SCRIPT=$(cat <<AWK_EOF
      /^[[:space:]]*jobs:/ { exit }
      /^[[:space:]]*\$/    { next }
      {
        if (match(\$0, /[^ \t]/)) { ind = RSTART - 1 } else { next }
        if (ind <= ${PUSH_SPACES}) { exit }
        if (ind == ${EXPECTED_CHILD_SPACES}) {
          if (\$0 ~ /^[ \t]*tags:([ \t]|\$)/)     tags++
          if (\$0 ~ /^[ \t]*branches:([ \t]|\$)/) branches++
        }
      }
      END { print (tags + 0) " " (branches + 0) }
AWK_EOF
    )

    CHILD_KEYS=$(printf '%s\n' "${AFTER_PUSH}" | awk "${AWK_SCRIPT}")
    HAS_TAGS=${CHILD_KEYS%% *}
    HAS_BRANCHES=${CHILD_KEYS##* }

    if [ "$HAS_TAGS" -eq 0 ]; then
      echo "::error::${RELEASE_YML} — push: trigger has no tags: as a direct child (DIST-05 violation)" >&2
      FAIL=1
    fi
    if [ "$HAS_BRANCHES" -gt 0 ]; then
      echo "::error::${RELEASE_YML} — push: trigger has both branches: and tags: as children; this publishes on every commit to main (DIST-05 violation)" >&2
      FAIL=1
    fi
  fi
fi

if [ "${FAIL}" -eq 0 ]; then
  echo "OK - release-artifact-source-gate: release.yml is the sole tag-triggered producer (DIST-05)."
fi

exit "${FAIL}"
