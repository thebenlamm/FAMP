#!/usr/bin/env bash
# DIST-04 gap closure: assert every release URL printed in the docs actually
# resolves.
#
# Why this exists: 16-04's `install_docs_accuracy.rs` asserts install-path
# ORDERING, the from-source fallback string, and the D-06 checksum wording --
# but nothing ever asserted that a documented download URL returns anything.
# The v1.1.0-rc.1 verification found all ten `/releases/latest/download/...`
# URLs returning 404, because GitHub's `/latest/` alias excludes pre-releases
# and the only published Release was one. The docs gate was green the whole
# time. A URL that 404s is the single most user-visible way onboarding docs
# can be wrong, so it gets a gate of its own.
#
# This is a NETWORK check, deliberately kept out of `cargo test`: unit tests
# must stay hermetic and offline-safe. It runs in CI via
# .github/workflows/install-docs-gate.yml, and locally via
# `just check-doc-release-urls`.
#
# Templates (URLs containing a `<placeholder>`) are skipped by design -- they
# are documentation of a FORM, not a fetchable address. They are reported so a
# silent skip can never be mistaken for a pass.

set -euo pipefail

DOCS=(README.md)
while IFS= read -r f; do DOCS+=("$f"); done < <(find docs -maxdepth 1 -name '*.md' | sort)

URL_RE='https://github\.com/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/releases/[^ )`"'"'"']+'

fail=0
checked=0
skipped=0

echo "-- checking release URLs printed in ${#DOCS[@]} doc file(s) --"

for doc in "${DOCS[@]}"; do
    [ -f "$doc" ] || continue
    while IFS= read -r line; do
        lineno="${line%%:*}"
        url="${line#*:}"

        # Strip trailing run of punctuation a URL can pick up from prose
        # (e.g., "URL." or "URL)," should become "URL").
        url="${url%%[.,;)]*}"

        if printf '%s' "$url" | grep -q '<[^>]*>'; then
            echo "  SKIP (template)  ${doc}:${lineno}  ${url}"
            skipped=$((skipped + 1))
            continue
        fi

        # -L: release asset URLs redirect to a signed CDN host.
        status=$(curl -sIL -o /dev/null -w '%{http_code}' --max-time 30 "$url" || echo "000")
        checked=$((checked + 1))

        if [ "$status" = "200" ]; then
            echo "  OK   ${status}  ${doc}:${lineno}  ${url}"
        else
            echo "  FAIL ${status}  ${doc}:${lineno}  ${url}" >&2
            fail=$((fail + 1))
        fi
    done < <(grep -noE "$URL_RE" "$doc" || true)
done

echo "-- checked ${checked} URL(s), skipped ${skipped} template(s), ${fail} failure(s) --"

if [ "$fail" -gt 0 ]; then
    cat >&2 <<'EOF'

ERROR: at least one documented release URL does not resolve.

A reader following these docs gets a dead command. Fix by making the URL
resolve, not by deleting the assertion.

Common cause: `/releases/latest/download/...` 404s whenever the newest
published Release is marked "pre-release" -- GitHub's `/latest/` alias
deliberately skips those. Either publish a non-prerelease Release, or point
the docs at a tag-pinned `/releases/download/<tag>/...` URL until one exists.

Note: both `/releases/latest/download/...` and `/releases/download/<tag>/...`
forms are accepted by the docs accuracy gate -- the invariant is that docs
lead with a prebuilt-binary installer URL, not which alias is used.
EOF
    exit 1
fi

if [ "$checked" -eq 0 ]; then
    echo "ERROR: no release URLs were checked -- the extraction regex matched nothing, which almost certainly means this gate is silently vacuous." >&2
    exit 1
fi

echo "OK - every documented release URL resolves."
