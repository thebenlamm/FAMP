#!/bin/sh
# Build FAMP on THIS computer from a commit delivered as a git bundle.
#
# It verifies the bundle, checks out the ref you name, proves the commit is the one you
# expect, builds, and prints the sha256 of each binary. It does not install anything and
# does not touch your PATH or any config. Be aware that cargo itself downloads and
# caches crates under CARGO_HOME (usually ~/.cargo) and runs build scripts; copying the
# binaries into place is a separate, manual step (printed at the end).
#
#   usage: famp-install-from-bundle.sh <bundle-file> <ref> <expected-commit-sha>
#   env:   FAMP_BUILD_DIR   where to build (default ~/famp-build; must not exist)
#          FAMP_LOCAL_REPO  a checkout that already has the bundle's prerequisite commits.
#                           REQUIRED for a RANGE bundle (e.g. v1.1.0-rc.2..branch), which
#                           cannot be cloned on its own; unnecessary for a full bundle.
#
# <ref> is a tag or branch name inside the bundle (or the commit sha itself). Build needs
# a C toolchain and cmake (aws-lc-sys, via rustls). The same steps as plain commands are
# in README.md ("Build on a box from a git bundle").
set -eu

[ "$#" -eq 3 ] || { sed -n '2,19p' "$0" >&2; exit 2; }
[ -f "$1" ] || { echo "no such bundle: $1" >&2; exit 1; }
# Absolute path: later steps run with a different working directory.
bundle=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
ref=$2 want=$3
work=${FAMP_BUILD_DIR:-$HOME/famp-build}

[ ! -e "$work" ] || { echo "$work already exists; move it aside or set FAMP_BUILD_DIR" >&2; exit 1; }

# `git bundle verify` needs a repository, and a fresh box has none: get one first.
if [ -n "${FAMP_LOCAL_REPO:-}" ]; then
  git clone --quiet "$FAMP_LOCAL_REPO" "$work"
  git -C "$work" bundle verify "$bundle"
  git -C "$work" fetch --quiet "$bundle" '+refs/tags/*:refs/tags/*' '+refs/heads/*:refs/remotes/bundle/*'
else
  git clone --quiet "$bundle" "$work" || {
    echo "Could not clone the bundle. A RANGE bundle (one that lists prerequisite commits) needs" >&2
    echo "FAMP_LOCAL_REPO=<a checkout that already has them>; see the header of this script." >&2
    exit 1
  }
  git -C "$work" bundle verify "$bundle"
fi

# The ref may be a tag, a branch fetched from the bundle, or the sha itself.
got=
for candidate in "$ref" "origin/$ref" "bundle/$ref"; do
  if got=$(git -C "$work" rev-parse --verify --quiet "$candidate^{commit}"); then break; fi
  got=
done
if [ "$got" != "$want" ]; then
  echo "REFUSING TO BUILD: '$ref' is '${got:-not found}', expected $want" >&2
  exit 1
fi
git -C "$work" checkout --quiet --detach "$want"
echo "$ref = $got (matches)"

command -v cmake >/dev/null 2>&1 || { echo "cmake is required to build (aws-lc-sys, via rustls); install it first" >&2; exit 1; }

cd "$work"
nice -n 19 cargo build --release --locked -j2 -p famp -p famp-gateway

for f in famp famp-gateway; do
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "target/release/$f"; else shasum -a 256 "target/release/$f"; fi
done
echo "Built in $work. To install, copy both binaries yourself, e.g.:"
echo "  install -m 0755 $work/target/release/famp $work/target/release/famp-gateway ~/.cargo/bin/"
