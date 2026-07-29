//! DOC-04 / D-07: accuracy gate — `docs/GATEWAY-SETUP.md`'s documented
//! `famp peer export`/`famp peer import` commands must match the shipping
//! clap CLI surface.
//!
//! Unlike `famp-gateway`, `famp peer` IS clap-based, so `--help` works
//! normally (mirrors `crates/famp/tests/cli_help_invariant.rs`'s pattern).
//! This asserts `famp peer export --help` / `famp peer import --help`
//! succeed and advertise the flags/subcommands the guide documents, and
//! that the guide's invocations are present in the clap help text.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
// The single test function below intentionally holds every accuracy
// assertion (flag-grep + the semantic checks added for the Gate A dogfood
// findings) in one place, mirroring the doc it validates section-by-section
// — splitting it into helpers would scatter each assertion's context away
// from the failure message that explains it.
#![allow(clippy::too_many_lines)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn gateway_setup_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/GATEWAY-SETUP.md")
}

#[test]
fn gateway_setup_doc_accuracy() {
    // `famp peer export --help`
    let export_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "export", "--help"])
        .output()
        .expect("famp peer export --help must be runnable");
    assert!(
        export_help.status.success(),
        "famp peer export --help must exit 0; got {:?}",
        export_help.status
    );
    let export_stdout = String::from_utf8_lossy(&export_help.stdout);
    assert!(
        export_stdout.contains("--as"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: expected `famp peer export --help` to advertise `--as`; got:\n{export_stdout}"
    );

    // `famp peer import --help`
    let import_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "import", "--help"])
        .output()
        .expect("famp peer import --help must be runnable");
    assert!(
        import_help.status.success(),
        "famp peer import --help must exit 0; got {:?}",
        import_help.status
    );

    // `famp peer --help` — confirms both subcommand names still exist.
    let peer_help = Command::cargo_bin("famp")
        .unwrap()
        .args(["peer", "--help"])
        .output()
        .expect("famp peer --help must be runnable");
    assert!(
        peer_help.status.success(),
        "famp peer --help must exit 0; got {:?}",
        peer_help.status
    );
    let peer_stdout = String::from_utf8_lossy(&peer_help.stdout);
    for verb in ["export", "import"] {
        assert!(
            peer_stdout
                .lines()
                .any(|l| l.trim_start().starts_with(verb)),
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: `famp peer --help` must advertise `{verb}`; got:\n{peer_stdout}"
        );
    }

    // Guide-content half: the guide must actually document these exact
    // invocations, not just the binary supporting them.
    let doc_path = gateway_setup_doc_path();
    let doc = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| {
        panic!(
            "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
             or the flag: could not read {}: {e}",
            doc_path.display()
        )
    });
    assert!(
        doc.contains("famp peer export --as"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: guide is missing `famp peer export --as` usage"
    );
    assert!(
        doc.contains("famp peer import"),
        "docs/GATEWAY-SETUP.md drifted from the shipping CLI — update the guide \
         or the flag: guide is missing `famp peer import` usage"
    );

    // -----------------------------------------------------------------
    // Semantic checks (Gate A dogfood, 10-HUMAN-UAT.md findings #1-#6/#8).
    // The flag-grep above only catches missing/renamed flags; these catch
    // the semantic inversions the grep missed — wrong wiring direction,
    // wrong pin-label authority, wrong ordering, and stale cert guidance.
    // Whitespace-normalize so a paragraph's markdown line-wrap can never
    // split an anchor phrase across a literal newline.
    // -----------------------------------------------------------------
    let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");

    // Finding #5/#8: "self-signed is fine" replaced with the CA:FALSE +
    // serverAuth recipe that verifies on both Apple SecTrust and webpki.
    assert!(
        doc.contains("CA:FALSE"),
        "update the guide or the code: guide must document the CA:FALSE \
         leaf-cert recipe (finding #8 — webpki rejects CA:TRUE as a server \
         end-entity)"
    );
    assert!(
        doc.contains("serverAuth"),
        "update the guide or the code: guide must document the serverAuth \
         EKU (finding #5 — Apple's verifier rejects a no-EKU cert)"
    );

    // D-05: own-domain config surface (plan 02) must be documented.
    assert!(
        doc.contains("FAMP_OWN_DOMAIN"),
        "update the guide or the code: guide must document the \
         FAMP_OWN_DOMAIN env var (own-domain config surface, plan 02)"
    );
    assert!(
        doc.contains("own-domain"),
        "update the guide or the code: guide must document the \
         $FAMP_HOME/own-domain file (own-domain config surface, plan 02)"
    );
    assert!(
        normalized.contains("famp send --to agent:"),
        "update the guide or the code: guide must document a \
         `famp send --to agent:<domain>/<name>` remote-addressing example \
         (plan 03)"
    );

    // Finding #6: macOS host firewall pre-authorization step.
    assert!(
        doc.contains("socketfilterfw"),
        "update the guide or the code: guide must document a macOS \
         socketfilterfw pre-auth step (finding #6 — the host firewall \
         silently drops unsolicited inbound TCP)"
    );

    // Finding #2: pin under the sender AGENT principal, never a
    // gateway-suffixed label. Positive: the guide names the sender agent
    // principal as the pin authority.
    assert!(
        normalized.contains("sender AGENT principal"),
        "update the guide or the code: guide §3 must instruct pinning \
         under the sender AGENT principal (finding #2 — ingress verifies \
         on the envelope `from`, an agent principal)"
    );
    // Negative: no pin label of the SHAPE `agent:<domain>/gateway` may
    // appear anywhere in the guide. Built from parts (never written
    // verbatim in this test file) and scoped so it does NOT false-positive
    // on legitimate filesystem paths like `~/.famp/gateway/identity.ed25519`
    // (no `agent:` prefix precedes those paths).
    let bad_pin_label_pattern = {
        let prefix = "agent:";
        let middle = r"[^\s]*/";
        let suffix = "gateway";
        format!("{prefix}{middle}{suffix}")
    };
    let bad_pin_label_re =
        regex::Regex::new(&bad_pin_label_pattern).expect("bad-pin-label pattern must compile");
    assert!(
        !bad_pin_label_re.is_match(&doc),
        "update the guide: found a gateway-suffixed pin label \
         (`agent:<domain>/gateway`-shaped) — finding #2 regression, pin \
         under the sender AGENT principal instead"
    );

    // Finding #1: wiring direction. A gateway backs the REMOTE principal it
    // proxies for, not the local one — concretely A backs bob, B backs
    // alice. Assert both CONCRETE directional statements, not just that
    // the word "back(s)" appears somewhere.
    assert!(
        normalized.contains("backs the remote principal `bob`"),
        "update the guide or the code: guide §4 must state that A's \
         gateway backs the remote principal `bob` (finding #1 — wiring \
         was inverted in the original guide)"
    );
    assert!(
        normalized.contains("backs the remote principal `alice`"),
        "update the guide or the code: guide §4 must state that B's \
         gateway backs the remote principal `alice` (finding #1 — wiring \
         was inverted in the original guide)"
    );

    // Findings #3/#4: keyring load-once + ready-after-keyring ordering.
    // The ready signal must be documented as appearing AFTER keyring load,
    // not merely mentioned somewhere in the doc — assert both presence and
    // document order.
    assert!(
        normalized.contains("keyring loads once"),
        "update the guide or the code: guide §3 must state the keyring \
         loads once at startup with no hot-reload (finding #3)"
    );
    let keyring_load_idx = normalized
        .find("loads its keyring")
        .expect("update the guide: §4 must describe keyring-load ordering relative to `ready`");
    let ready_idx = normalized
        .find("famp-gateway: ready")
        .expect("update the guide: §4 must document the `famp-gateway: ready` signal");
    assert!(
        keyring_load_idx < ready_idx,
        "update the guide or the code: the `famp-gateway: ready` signal \
         must be documented as appearing AFTER keyring load (finding #4 — \
         a ready line before keyring load is a false-success signal); \
         found keyring-load mention at byte {keyring_load_idx}, ready \
         mention at byte {ready_idx}"
    );
    assert!(
        normalized.contains("after the keyring has loaded prints"),
        "update the guide or the code: guide §4 must explicitly state the \
         ready line prints AFTER the keyring has loaded (finding #4)"
    );
}
