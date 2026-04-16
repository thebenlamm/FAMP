---
name: famp-setup
description: Set up FAMP identity for Claude Code MCP integration. Creates identity, selects port, outputs peer card for sharing with other agents.
---

# FAMP Setup for Claude Code

One-command setup for FAMP MCP integration with Claude Code.

## What This Does

1. Creates a FAMP identity (Ed25519 keypair, TLS cert)
2. Selects an available port (auto-detects conflicts)
3. Configures the agent with a unique principal
4. Outputs a peer card for sharing with other agents

## Invocation

```
/famp-setup                    — Set up default agent named "self"
/famp-setup alice              — Set up agent named "alice"
/famp-setup --connect          — Set up and connect to another agent
```

## Workflow

### Step 1: Check Current State

First, check if this session already has FAMP configured:

```bash
ls -la $FAMP_HOME 2>/dev/null || echo "No FAMP_HOME set"
```

If FAMP_HOME is set and initialized, use `famp info` to show the existing peer card.
If not configured, proceed to Step 2.

### Step 2: Determine Agent Name

If the user provided a name (e.g., `/famp-setup alice`), use it.
Otherwise, ask using AskUserQuestion:

> What name should this agent use? (e.g., "alice", "bob", "main")
> This will be used for the principal (agent:localhost/<name>) and suggested alias.

### Step 3: Run Setup

Execute the setup command:

```bash
# Build if needed
cd /home/ubuntu/Workspace/FAMP && cargo build --release 2>&1 | tail -3

# Run setup with explicit home to avoid conflicts
./target/release/famp setup --name <NAME> --home /tmp/famp-<NAME>
```

### Step 4: Configure MCP (if needed)

Check if `.mcp.json` exists in the project root. If so, offer to add this agent:

```json
{
  "mcpServers": {
    "famp-<NAME>": {
      "command": "/home/ubuntu/Workspace/FAMP/target/release/famp",
      "args": ["mcp"],
      "env": { "FAMP_HOME": "/tmp/famp-<NAME>" }
    }
  }
}
```

Tell the user they need to restart Claude Code for MCP changes to take effect.

### Step 5: Share Peer Card

The setup command outputs a peer card JSON. Tell the user:

> **Your peer card (share this with other agents):**
> ```json
> <peer card output>
> ```
>
> To connect with another agent:
> 1. Get their peer card JSON
> 2. Run: `echo '<their-card>' | FAMP_HOME=/tmp/famp-<NAME> famp peer import`

### Step 6: Start Daemon (optional)

Ask if the user wants to start the daemon now:

```bash
FAMP_HOME=/tmp/famp-<NAME> ./target/release/famp listen &
```

Note: The daemon must be running for other agents to send messages to this one.

## Connect Mode (--connect)

If invoked with `--connect`, after setup:

1. Ask for the other agent's peer card JSON
2. Import it: `echo '<card>' | FAMP_HOME=/tmp/famp-<NAME> famp peer import`
3. Show how to send a test message via MCP

## Troubleshooting

### Port Already in Use

If setup fails with port conflict, specify a port explicitly:
```bash
./target/release/famp setup --name <NAME> --home /tmp/famp-<NAME> --port 8444
```

### MCP Not Working

1. Check the daemon is running: `ps aux | grep "famp listen"`
2. Check MCP config is correct in `.mcp.json`
3. Restart Claude Code after MCP config changes

### Peer Import Failed

Ensure the peer card JSON is valid:
```bash
echo '{"alias":"bob","endpoint":"https://127.0.0.1:8443","pubkey":"...","principal":"agent:localhost/bob"}' | jq .
```
