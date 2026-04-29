#!/usr/bin/env bash
# MCP-01 audit (D-11): assert MCP/bus/broker source paths do not import reqwest or rustls.
# Phase 2 source-import grep — Phase 4 lets the cargo-tree audit follow once the
# federation CLI surfaces are deleted.
set -euo pipefail
if grep -rE 'use (reqwest|rustls)' \
    crates/famp/src/cli/mcp/ \
    crates/famp/src/bus_client/ \
    crates/famp/src/broker/ 2>/dev/null; then
  echo "MCP-01 violation: MCP/bus/broker source imports federation transports" >&2
  exit 1
fi
echo "MCP-01: OK — no reqwest/rustls imports under cli/mcp/, bus_client/, or broker/"
