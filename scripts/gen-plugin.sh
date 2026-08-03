#!/usr/bin/env bash
# Regenerate the derived files under plugins/<host>/ from the canonical assets
# in crates/famp/assets/.
#
#   usage: scripts/gen-plugin.sh [host]     (default: claude-code)
#
# ── Why anything is generated at all ──────────────────────────────────────────
#
# All three hosts FAMP targets — Claude Code, Codex, and Grok Build — now ship a
# plugin marketplace, and all three bundle roughly the same component set
# (skills/commands, hooks, an MCP server declaration). What differs between them
# is the manifest and, critically, **how a plugin-provided MCP server's tools are
# named when exposed to the model**.
#
# The slash commands name their tools explicitly, in `allowed-tools` frontmatter
# and in the body prose. So the same command asset cannot be shipped verbatim to
# a host whose namespacing differs — hence a per-host rewrite rather than a copy.
#
# ── Namespacing, per host ─────────────────────────────────────────────────────
#
# All three hosts differ, and all three rules below are grounded in either a live
# session or the host's own source. Guessing here is uniquely bad: a wrong
# namespace yields commands whose `allowed-tools` match no real tool, which fails
# silently at runtime rather than erroring.
#
#   claude-code  `mcp__plugin_<plugin>_<server>__<tool>`
#                RUNTIME-VERIFIED. Loaded the plugin and enumerated tools:
#
#                  $ claude --plugin-dir ./plugins/claude-code \
#                      -p "list the exact names of every tool matching 'famp'"
#                  mcp__plugin_famp_famp__famp_register
#
#   codex        `mcp__<server>__<tool>` — identical to the bare namespace the
#                standalone `install-codex` path already produces, so the rewrite
#                is a deliberate no-op.
#                SOURCE-DERIVED (openai/codex):
#                  codex-mcp/src/mcp/mod.rs   qualified_mcp_tool_name_prefix() =
#                                             "mcp" + "__" + server_name + "__"
#                  core-plugins/src/loader.rs plugin MCP *server* names are not
#                                             prefixed — kept as declared and
#                                             deduped globally ("skipping
#                                             duplicate plugin MCP server name")
#                  utils/plugins/…/plugin_namespace.rs
#                                             `plugin_namespace` is consumed only
#                                             by core-skills, never by MCP naming
#
#   grok         `<server>__<tool>` — no `mcp__` prefix at all.
#                SOURCE-DERIVED (xai-org/grok-build):
#                  xai-grok-mcp/src/servers.rs      tool meta keyed by
#                                                   "{server}{__}"
#                  docs/user-guide/22-permissions-and-safety.md
#                    "Grok tool names carry no `mcp__` prefix, so a rule written
#                     as `mcp__server__tool` never matches an MCP call; write
#                     `MCPTool(server__tool)` instead."
#
# codex and grok are source-derived, not runtime-verified — neither CLI could be
# driven to enumerate tools without provider credentials. Flagged in the PR.
#
# ── What is derived ───────────────────────────────────────────────────────────
#
#   commands/*.md   rewritten tool namespace, and renamed `famp-send.md` ->
#                   `send.md` because plugin skills are already namespaced by
#                   the plugin (`/famp:send`, not `/famp:famp-send`).
#   hooks/*.sh      rendered from crates/famp/assets/. Canonical assets pin the
#                   binary via `FAMP_BIN=@FAMP_BIN@` (see #28 / install path).
#                   Plugins have no install-time render step, so this script
#                   substitutes a host-specific value:
#
#                     claude-code  FAMP_BIN="${CLAUDE_PLUGIN_ROOT}/bin/famp"
#                                  (plugin bin/ resolver shim on PATH)
#                     codex|grok   FAMP_BIN=famp
#                                  (bare name; /famp:setup puts it on PATH)
#
#                   After render, unresolved `@FAMP_BIN@` in hooks/ is a hard
#                   error, and each shim is syntax-checked with `bash -n`.
#
# Outputs are committed, because an installed plugin is a git clone and must
# contain real files. `just plugin-check` / `just plugin-check-all` fail if
# they have drifted.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$REPO_ROOT/crates/famp/assets"

HOST="${1:-claude-code}"

# Namespace each host substitutes for the bare `mcp__famp__` used by the
# standalone `install-*` commands. See the block above for provenance.
case "$HOST" in
  claude-code) SCOPED_NS="mcp__plugin_famp_famp__" ;;
  codex)       SCOPED_NS="mcp__famp__" ;;   # no-op by design; see above
  grok)        SCOPED_NS="famp__" ;;
  *)
    echo "error: unknown host '$HOST' (expected: claude-code, codex, grok)" >&2
    exit 2
    ;;
esac

PLUGIN="$REPO_ROOT/plugins/$HOST"
BARE_NS="mcp__famp__"

if [ ! -d "$PLUGIN" ]; then
  echo "error: $PLUGIN does not exist" >&2
  exit 4
fi

mkdir -p "$PLUGIN/commands" "$PLUGIN/hooks"

echo "host: $HOST"
echo "namespace: $BARE_NS -> $SCOPED_NS"
echo "regenerating $PLUGIN/commands/"
for src in "$ASSETS"/slash_commands/famp-*.md; do
  base="$(basename "$src" .md)"      # famp-send
  short="${base#famp-}"              # send
  sed "s/${BARE_NS}/${SCOPED_NS}/g" "$src" > "$PLUGIN/commands/$short.md"
  echo "  $base.md -> commands/$short.md"
done

# Fail loudly if any bare reference survived — a missed rewrite produces a
# command whose allowed-tools entry matches nothing, which fails silently at
# runtime and is exactly the class of bug this script exists to prevent.
#
# Skipped for codex, whose namespace *is* the bare one: there the rewrite is a
# deliberate no-op and surviving `mcp__famp__` references are correct.
if [ "$SCOPED_NS" != "$BARE_NS" ]; then
  if grep -rl "${BARE_NS}" "$PLUGIN/commands" 2>/dev/null | grep -q .; then
    echo "error: un-rewritten ${BARE_NS} references remain in $PLUGIN/commands" >&2
    grep -rn "${BARE_NS}" "$PLUGIN/commands" >&2
    exit 5
  fi
else
  # Positive assertion instead: the bare namespace must be present and intact.
  if ! grep -rq "${BARE_NS}" "$PLUGIN/commands" 2>/dev/null; then
    echo "error: expected bare ${BARE_NS} references in $PLUGIN/commands, found none" >&2
    exit 5
  fi
fi

# Host-specific render of the #28 asset token. Plugins have no install-time
# step, so this is the only place @FAMP_BIN@ becomes a real assignment.
echo "rendering Stop-hook shims to $PLUGIN/hooks/"
for shim in hook-runner.sh famp-await.sh; do
  src="$ASSETS/$shim"
  dest="$PLUGIN/hooks/$shim"

  if ! grep -q '^FAMP_BIN=@FAMP_BIN@$' "$src"; then
    echo "error: expected a single 'FAMP_BIN=@FAMP_BIN@' assignment in $src" >&2
    exit 6
  fi

  case "$HOST" in
    claude-code)
      # Expand CLAUDE_PLUGIN_ROOT at hook runtime (same token hooks.json uses).
      # SC2016 is expected here: the single quotes are the point. This must
      # emit a LITERAL ${CLAUDE_PLUGIN_ROOT} into the generated hook so it
      # expands when the hook runs, not when this generator runs. Expanding it
      # here would bake in the generating machine's path.
      # shellcheck disable=SC2016
      sed 's|^FAMP_BIN=@FAMP_BIN@$|FAMP_BIN="${CLAUDE_PLUGIN_ROOT}/bin/famp"|' \
        "$src" > "$dest"
      ;;
    codex|grok)
      # No plugin bin/ PATH injection; setup puts a real `famp` on PATH.
      sed 's|^FAMP_BIN=@FAMP_BIN@$|FAMP_BIN=famp|' \
        "$src" > "$dest"
      ;;
  esac

  chmod 0755 "$dest"

  if grep -q '@FAMP_BIN@' "$dest"; then
    echo "error: unresolved @FAMP_BIN@ remains in $dest" >&2
    exit 7
  fi

  if ! bash -n "$dest"; then
    echo "error: bash -n failed for $dest" >&2
    exit 8
  fi

  echo "  $shim"
done

# Scoped token guard: only generated hooks (not docs that may mention the token).
if grep -rq '@FAMP_BIN@' "$PLUGIN/hooks" 2>/dev/null; then
  echo "error: unresolved @FAMP_BIN@ remains under $PLUGIN/hooks" >&2
  grep -rn '@FAMP_BIN@' "$PLUGIN/hooks" >&2 || true
  exit 7
fi

echo "done."
