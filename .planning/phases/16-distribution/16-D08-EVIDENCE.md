# D-08 Evidence: x86_64-apple-darwin cross-build on arm64 macos-latest

**Run URL:** https://github.com/thebenlamm/FAMP/actions/runs/30782329455

**Runner:** `macos-latest` resolved to image `macos-26-arm64` (version `20260728.0273.1`), macOS `26.5.2`.

**`uname -m`:** `arm64`

**`cmake --version`:** `cmake version 4.4.0` (CMake suite maintained and supported by Kitware)

**Run conclusion:** `success`

**Steps executed cleanly, no `continue-on-error`:**
1. `rustup target add x86_64-apple-darwin` — succeeded.
2. `cargo build --release -p famp -p famp-gateway -p famp-relay --target x86_64-apple-darwin` — succeeded (the `aws-lc-sys` cmake invocation cross-compiled without error, despite the workspace's actually-resolved `aws-lc-rs` crypto backend per `Cargo.lock`).
3. Final assertion step printed `all three binaries present` — `target/x86_64-apple-darwin/release/{famp,famp-gateway,famp-relay}` all exist and are executable.

## Resolution: D-08a

A single `macos-latest` (arm64) runner cross-compiles `x86_64-apple-darwin` cleanly for all three
shipping binaries. No second (Intel) macOS runner is required. `16-RESEARCH.md` Fork C's
recommendation is confirmed by a real build log, not by reading; `famp-lead-730`'s native-matrix
hypothesis (labelled by them as "a starting hypothesis to verify") is not needed here.

This resolves the disagreement: `[workspace.metadata.dist]`'s `targets` list may include both
`aarch64-apple-darwin` and `x86_64-apple-darwin` under the default `macos-latest` runner, with no
`github-custom-runners` override needed for either darwin triple.
