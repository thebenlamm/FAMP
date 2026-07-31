#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! QUAR-05 / D-22 non-vacuity control for the mechanical rendering-surface
//! regression gate (`just check-quarantine-surfaces`,
//! `scripts/quarantine-surfaces.sh --check`).
//!
//! A regression gate whose own "goes red" behavior is untested is not a
//! gate anyone is actually checking (14-03-PLAN.md Task 2). This test
//! proves the gate is not vacuous end to end: green on the current tree,
//! red with a fabricated unregistered call site present and named in the
//! output, green again once it is removed. Deliberately NOT `#[ignore]` —
//! it must run in the normal suite.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;

/// Serializes this test against any future sibling test that might also
/// write into `crates/famp/src/` — writing into the source tree is
/// process-tree-global state, unlike most of this crate's tests which
/// isolate themselves via `tempfile`/per-test sockets.
static QUARANTINE_GATE_LOCK: Mutex<()> = Mutex::new(());

fn repo_root() -> PathBuf {
    // crates/famp -> crates -> repo root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn run_gate(root: &Path) -> Output {
    Command::new("sh")
        .arg("scripts/quarantine-surfaces.sh")
        .arg("--check")
        .current_dir(root)
        .output()
        .expect("failed to spawn scripts/quarantine-surfaces.sh")
}

/// RAII guard for a fabricated production-path source file: deletes it on
/// drop, including on panic-driven unwind, mirroring this repo's
/// `ChildGuard` convention (crates/famp/tests/common/child_guard.rs) of
/// never leaving test-created state behind after a failure.
struct TempSourceFile {
    path: PathBuf,
}

impl TempSourceFile {
    fn create(path: PathBuf, contents: &str) -> Self {
        fs::write(&path, contents).expect("write temp probe source file");
        Self { path }
    }
}

impl Drop for TempSourceFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[test]
fn gate_goes_red_on_an_unregistered_surface() {
    let _lock = QUARANTINE_GATE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = repo_root();

    // 1. Green on the current, unmodified tree.
    let before = run_gate(&root);
    assert!(
        before.status.success(),
        "gate should be green before the probe file exists: stdout={} stderr={}",
        String::from_utf8_lossy(&before.stdout),
        String::from_utf8_lossy(&before.stderr),
    );

    // 2. Fabricate an unregistered call site matching the `envelope-body`
    // family (a `.body()` call in a file that references `EnvelopeView`),
    // under a name that is obviously temporary and not committed anywhere.
    let probe_name = "quarantine_gate_test_temp_probe.rs";
    let probe_path = root.join("crates/famp/src").join(probe_name);
    let probe_src = "use famp_envelope::EnvelopeView;\n\
pub fn quarantine_gate_temp_probe(v: &serde_json::Value) {\n    \
        let view = EnvelopeView::new(v);\n    \
        let _ = view.body();\n\
}\n";
    let guard = TempSourceFile::create(probe_path, probe_src);

    // 3. The gate must now fail and name the fabricated file in its output.
    let during = run_gate(&root);
    assert!(
        !during.status.success(),
        "gate should exit nonzero while the unregistered probe file exists"
    );
    let during_stdout = String::from_utf8_lossy(&during.stdout);
    let during_stderr = String::from_utf8_lossy(&during.stderr);
    assert!(
        during_stdout.contains(probe_name) || during_stderr.contains(probe_name),
        "gate failure output should name the fabricated file {probe_name}: stdout={during_stdout} stderr={during_stderr}"
    );

    // 4. Remove the fabricated file; the gate must go green again.
    drop(guard);
    let after = run_gate(&root);
    assert!(
        after.status.success(),
        "gate should be green again once the probe file is removed: stdout={} stderr={}",
        String::from_utf8_lossy(&after.stdout),
        String::from_utf8_lossy(&after.stderr),
    );
}
