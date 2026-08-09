# Follower Setup: Ben and a Second Person

This is the complete happy path for two independently administered machines on
different networks. Ben is the **inviter**; the second person is the
**follower/redeemer**. Do not use a shared VPN, copy private keys, paste peer
key blobs, or build from source for this acceptance run. A successful `famp
send` means only that the sender's local gateway accepted the envelope; it is
not proof of remote delivery or completion.

## 1. Install the published release on both machines

On Ben's machine and then on the follower's supported macOS or Linux machine:

```sh
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh
curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-gateway-installer.sh | sh
famp --version
command -v famp-gateway
famp daemon install
```

`famp-gateway` has no `--help` (it's a hand-rolled parser, not clap) and no
`--version`; every real invocation needs `--listen`, `--tls-cert`,
`--tls-key`, and a running broker, so the install check is a `PATH` presence
check that prints the installed path.

Use the prebuilt installers. Rust and a source checkout are not part of this
path. If an installer reports that `~/.cargo/bin` is missing from `PATH`, apply
its printed shell-profile instruction, open a new shell, and rerun the `famp
--version` and `command -v famp-gateway` checks.

**Linux only — keep the broker alive after you log out.** `famp daemon install`
installs a systemd `--user` service, and systemd stops user services when your
last session ends. On a machine you reach only over SSH that means the broker
dies the moment you disconnect, and the later sections fail with no obvious
cause. `famp daemon install` detects this and prints the exact remedy without
running it: if it reports that linger is not enabled, run the
`loginctl enable-linger <user>` command it printed, then confirm with
`famp daemon status` that the reported state includes `linger=yes`. macOS has
no equivalent step — its launchd LaunchAgent needs nothing extra.

## 2. Start reachable gateways

The follower's pairing redemption in section 3 (`crates/famp/src/cli/pair/redeem.rs`)
POSTs the signed pairing code directly to `<url>/famp/v1/pair/redeem` over
HTTPS with a 10-second timeout — a direct dial of the inviter's gateway,
never relay-carried. So before section 3, Ben's gateway URL must be
inbound-reachable from the follower's network. A relay carries ongoing
message transport, not pairing, so deploying one does not remove this
requirement. The follower dials out during pairing and so does not need an
inbound-reachable endpoint for section 3, but still needs a running gateway
of their own for sections 5 and 6.

Sections 5 and 6 additionally need message transport in both directions:
either both gateways inbound-reachable to each other, or both pointed at a
relay — see [Relay Setup](RELAY-SETUP.md) for that procedure.

[Gateway Setup](GATEWAY-SETUP.md) is still the reference for the mechanical
parts: its section 1 TLS certificate recipe and macOS inbound-firewall step,
its section 4 flag surface and ready signal, and its section 5 own-domain
configuration. Skip its section 3 out-of-band key exchange entirely — `famp
pair` in section 3 below replaces it, and this run must not move key
material by hand.

**Create the empty peer keyring before the first gateway start.** On a machine
that has never pinned a peer the file does not exist yet, and the released
`v1.1.0-rc.1` gateway exits with `failed to load peers keyring … No such file
or directory` instead of starting (issue #42). An empty file is a valid
keyring — it pins nobody, which is the correct state before pairing:

```sh
mkdir -p ~/.famp/gateway && touch ~/.famp/gateway/peers.keyring
```

Do not continue until each owner sees the gateway's ready signal for their
own endpoint.

## 3. Ben invites; the follower redeems

Ben, the inviter, confirms the follower's `famp --version` worked, then runs:

```sh
famp pair invite --as agent:<ben-domain>/<ben-name> --url https://<ben-gateway> --confirm-installed
```

Ben sends the entire printed artifact to the follower. The follower is the
redeemer and runs the artifact's command:

> **Pairing gives the other person the same trust as someone at your terminal.**
> Only continue if you know who sent this invitation and intended to connect
> these two machines.

```sh
famp pair redeem --from https://<ben-gateway> --as <follower-name>
```

The follower types the five-word code only at the prompt.

**If you see "Could not reach {url}":** this usually means Ben's gateway is not running or not reachable. But if it IS running and the error persists, it might be a TLS trust issue — Ben's certificate might be self-signed or issued by a CA your machine does not trust. In that case, Ben can provide you with the certificate file, and you re-run with `--trust-cert <path-to-cert>`. Ben, still the
inviter, then observes the redeemer identity before accepting the pin:

```sh
famp pair status
```

Ben confirms the displayed follower principal and key identifier. Pairing is
asymmetric: redemption pins Ben on the follower machine, while Ben's `status`
confirmation pins the follower on Ben's machine.

## 4. Restart the gateway after pinning and wait for fresh readiness

Pinned keyrings load once at gateway startup. Both owners restart their gateway
after the pins are written, then each waits for a fresh ready signal. Do not
send either task using a readiness line emitted before pairing.

**Restart the gateway itself, not the broker.** `famp daemon restart` restarts
only the broker; it does **not** restart a gateway, and there is no
`famp daemon` command that does. Running it here leaves the gateway holding the
empty keyring it loaded before pairing, and section 5 then fails with no error
message at all — the task simply never arrives.

`famp-gateway` is the plain foreground process each owner started in section 2.
To restart it: press `Ctrl-C` in the terminal running it, then run **the exact
same command again** — same `--listen`, same `--tls-cert`/`--tls-key`, same
`--peer`/`--relay-fetch`/`--backs`, same trailing principal name. Changing any
flag here changes what you are testing.

Wait for a **new** ready line before continuing:

```text
famp-gateway: ready, backing N principal(s): <names>
```

A `ready` line printed before the pins were written does not count. If you
started your gateway under a service manager you wrote yourself, restart it
through that instead — the requirement is a fresh process, not a particular
mechanism. Troubleshoot startup or reachability in
[Gateway Setup](GATEWAY-SETUP.md); do not replace this path with a shared VPN.

## 5. Task A: Ben sends, follower receives and closes

Ben registers his local identity in a separate terminal (or backgrounded). **Keep that terminal or background process running through section 6.** If you Ctrl-C it, every later `--as` command will fail `NotRegistered` because the broker requires a live canonical holder:

```sh
famp register <ben-name>
```

In a different terminal (on the same machine), send a new task to the follower:

```sh
famp send --as <ben-name> --to agent:<follower-domain>/<follower-name> --new-task "phase20-ben-to-follower" --body "Reply with the requested result"
```

Ben records the returned task ID as `<ben-to-follower-task-id>`. The zero exit
status is local acceptance only. Because remote-origin traffic intentionally
does not auto-wake `famp await`, the follower explicitly lists the Inbox.

The follower also registers in a separate terminal and keeps it running:

```sh
famp register <follower-name>
```

In a different terminal, the follower checks the Inbox:

```sh
famp inbox list --as <follower-name>
famp send --as <follower-name> --to agent:<ben-domain>/<ben-name> --task <ben-to-follower-task-id> --body <result> --terminal
famp inspect tasks --id <ben-to-follower-task-id> --json
```

The follower—the receiving owner—captures the final inspection and confirms
the task state is exactly `COMPLETED`, `FAILED`, or `CANCELLED`.

For a host agent, the equivalent explicit path is: the follower calls
`famp_inbox`, then calls `famp_send` in `reply` mode with the same task ID. Reply
mode closes terminally by default; it must not wait for an automatic wake.

## 6. Task B: follower sends, Ben receives and closes

The follower opens a different task in the opposite direction:

```sh
famp send --as <follower-name> --to agent:<ben-domain>/<ben-name> --new-task "phase20-follower-to-ben" --body "Reply with the requested result"
```

The follower records the returned task ID as
`<follower-to-ben-task-id>`; it must differ from Task A. Ben explicitly
processes his Inbox and closes this second task:

```sh
famp inbox list --as <ben-name>
famp send --as <ben-name> --to agent:<follower-domain>/<follower-name> --task <follower-to-ben-task-id> --body <result> --terminal
famp inspect tasks --id <follower-to-ben-task-id> --json
```

Ben—the receiving owner—captures the final inspection and confirms exactly one
terminal state: `COMPLETED`, `FAILED`, or `CANCELLED`. Sender-side output or a
report relayed by the sender does not substitute for receiver-owned proof.

For a host agent, Ben calls `famp_inbox`, then `famp_send` in `reply` mode with
the same Task B ID and the default terminal close.

## 7. Pairing-message comprehension review (human judgment remains open)

Before calling the run accepted, the follower reviews and paraphrases these
seven shipped, non-mutating failure messages. Automation keeps them synchronized
with `PairingError`; it does **not** measure comprehension or close PAIR-05.

1. `That does not look like a pairing code. A pairing code is exactly five lowercase words separated by spaces. Check the message you were sent and type it again.`
2. `This code has expired. Codes last 24 hours. Ask the person who invited you to send a new one.`
3. `This code has already been used. If that was not you, tell the person who invited you right away and ask them to run: famp pair revoke --all-pending`
4. `Too many wrong tries, so this code is now locked. Ask the person who invited you to send a new one.`
5. `That code did not match. Check for a typo, then try again. If you run out of tries, ask the person who invited you to send a new code.`
6. `Could not reach {url}. Check that you copied the address exactly, then ask the person who invited you whether their FAMP gateway is running.`
7. `This code cannot be redeemed on the same machine that created it. Run this on the machine you want to connect.`

Record each first paraphrase without coaching. The human reviewer, not this
guide's automated accuracy test, judges whether it is actionable.
