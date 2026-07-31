#!/bin/sh
# scripts/quarantine-surfaces.sh
#
# QUAR-02 / QUAR-05 / D-05 / D-22: mechanical enumeration of every
# production call site that can reach received (agent-authored) content,
# so the rendering-surface list is generated, never hand-curated. The
# hand-curated list already failed its own standard once (it missed
# `register --tail` and `wait-reply` — see 14-CONTEXT.md D-04). This
# script is the single source of truth: both the checked-in allowlist
# (.quarantine-surfaces.allow) and the QUAR-05 regression gate
# (`just check-quarantine-surfaces`, crates/famp/tests/quarantine_gate.rs)
# consume its output verbatim rather than re-deriving the list (D-05).
#
# SCOPE (deliberate, not an oversight — T-14-15):
#   - Only `crates/**/*.rs`.
#   - Excludes any `*/tests/*` directory tree-wide (not just
#     crates/famp/tests/ — every crate's own integration-test directory is
#     equally test scaffolding, not a production render path).
#   - Excludes any `#[cfg(test)] mod ... { }` block embedded in a scanned
#     source file (unit-test fixtures living inline in src/, e.g.
#     register.rs's own `mod tests`), stripped by brace-depth tracking.
#   - Excludes comment-only lines (first non-whitespace characters begin a
#     `//` line comment), stripped BEFORE the counting greps below, so a
#     doc comment that merely mentions one of these tokens (e.g.
#     famp-gateway/src/egress.rs's doc comment about `.body()`) cannot
#     register as a call site. This is the forbidden
#     bare-`grep -c`-on-an-unfiltered-file pattern's fix.
#
# FOUR DETECTED FAMILIES (each tagged with a distinct `kind` token):
#   envelope-body   - a `.body()` call in a file that references
#                      `EnvelopeView` (the only type in this tree exposing
#                      that accessor over received envelope content; the
#                      per-file gate keeps famp-envelope's own internal
#                      `BusEnvelope<B>::body()` — a different, typed
#                      accessor — out of scope).
#   emit-tail-line  - a call site invoking `emit_tail_line(...)`.
#   write-outcome   - a call site invoking `write_outcome(...)`.
#   envelopes-field - a JSON-literal output site keying `"envelopes":` or
#                      `"drained":` with a value that is not a bare
#                      `.len()` count. The quoted-key form distinguishes
#                      external JSON output construction from internal
#                      Rust struct field declarations of the same name
#                      (`envelopes: Vec<StampedEnvelope>` — no quotes,
#                      famp-bus/famp-inspect-proto plumbing); the `.len()`
#                      exclusion distinguishes an envelope *count* (safe,
#                      several MCP tools report `"drained": drained.len()`)
#                      from actual envelope *values* per this family's
#                      definition in 14-03-PLAN.md Task 1.
#
# Every record is `path:line:kind`, one per line, sorted.
#
# USAGE:
#   scripts/quarantine-surfaces.sh            regenerate: print the list, exit 0
#   scripts/quarantine-surfaces.sh --check    diff against .quarantine-surfaces.allow;
#                                              exit 1 with the diff + a QUAR-05
#                                              remediation note on drift

set -eu

SELF_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH='' cd -- "${SELF_DIR}/.." && pwd)
ALLOWLIST="${ROOT_DIR}/.quarantine-surfaces.allow"

# Strip #[cfg(test)] mod blocks and comment-only lines from a file.
# Emits "lineno:content" for every surviving line.
strip_noise() {
  awk '
    BEGIN { skip = 0; depth = 0; pending = 0 }
    {
      raw = $0
      trimmed = raw
      sub(/^[ \t]+/, "", trimmed)
      if (skip) {
        opens = gsub(/\{/, "{", raw)
        closes = gsub(/\}/, "}", raw)
        depth += opens - closes
        if (depth <= 0) { skip = 0 }
        next
      }
      if (pending) {
        if (trimmed ~ /^#\[/) { next }
        if (trimmed ~ /^mod[ \t]+[A-Za-z0-9_]+[ \t]*\{/) {
          pending = 0
          skip = 1
          depth = 1
          next
        }
        pending = 0
      }
      if (trimmed ~ /^#\[cfg\(test\)\]/) { pending = 1; next }
      if (trimmed ~ /^\/\// ) { next }
      print NR ":" raw
    }
  ' "$1"
}

emit_records() {
  find "${ROOT_DIR}/crates" -type f -name '*.rs' -not -path '*/tests/*' | while IFS= read -r f; do
    rel=${f#"${ROOT_DIR}"/}
    has_envelope_view=0
    if grep -q 'EnvelopeView' "$f" 2>/dev/null; then
      has_envelope_view=1
    fi
    strip_noise "$f" | while IFS=: read -r lineno content; do
      if [ "$has_envelope_view" = "1" ]; then
        case "$content" in
          *.body\(\)*) printf '%s:%s:envelope-body\n' "$rel" "$lineno" ;;
        esac
      fi
      case "$content" in
        *emit_tail_line\(*) printf '%s:%s:emit-tail-line\n' "$rel" "$lineno" ;;
      esac
      case "$content" in
        *write_outcome\(*) printf '%s:%s:write-outcome\n' "$rel" "$lineno" ;;
      esac
      case "$content" in
        *\"envelopes\":*|*\"drained\":*)
          case "$content" in
            *.len\(\)*) ;;
            *) printf '%s:%s:envelopes-field\n' "$rel" "$lineno" ;;
          esac
          ;;
      esac
    done
  done
}

generated=$(emit_records | sort -u)

if [ "${1:-}" = "--check" ]; then
  current=$(grep -v '^[[:space:]]*#' "${ALLOWLIST}" | grep -v '^[[:space:]]*$' || true)
  if [ "${generated}" = "${current}" ]; then
    echo "OK - quarantine surface list matches .quarantine-surfaces.allow (QUAR-05)."
    exit 0
  fi
  echo "QUAR-05 VIOLATION: the mechanical rendering-surface query no longer matches .quarantine-surfaces.allow." >&2
  echo "A call site was added or removed without updating the allowlist." >&2
  echo "" >&2
  echo "diff (- = missing from allowlist / removed surface, + = new unregistered surface):" >&2
  allow_tmp=$(mktemp)
  gen_tmp=$(mktemp)
  trap 'rm -f "${allow_tmp}" "${gen_tmp}"' EXIT
  printf '%s\n' "${current}" > "${allow_tmp}"
  printf '%s\n' "${generated}" > "${gen_tmp}"
  diff -u "${allow_tmp}" "${gen_tmp}" >&2 || true
  rm -f "${allow_tmp}" "${gen_tmp}"
  trap - EXIT
  echo "" >&2
  echo "Remedy: route the new call site through crates/famp/src/cli/render.rs" >&2
  echo "(render_envelope_body / render_body_text), OR — only if it genuinely" >&2
  echo "renders no received content — regenerate this file and add a" >&2
  echo "justification comment above the new record explaining why." >&2
  exit 1
fi

printf '%s\n' "${generated}"
