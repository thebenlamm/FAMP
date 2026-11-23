#!/bin/sh
# Build FAMP on THIS computer from a tagged commit delivered as a git bundle.
# Installs nothing and touches nothing outside the build directory: it verifies the
# bundle, checks out the tag, proves the commit is the one you expect, builds, and
# prints the sha256 of each binary. Copying them into place is a separate, manual
# step (printed at the end).
#
#   usage: famp-install-from-bundle.sh <bundle-file> <tag> <expected-commit-sha>
#   env:   FAMP_BUILD_DIR  where to build (default ~/famp-build; must not exist)
#
# The equivalent plain commands are listed in README.md ("Build on a box from a
# git bundle"); this script is only those commands in order.
set -eu

[ "$#" -eq 3 ] || { sed -n '2,12p' "$0" >&2; exit 2; }
bundle=$1 tag=$2 want=$3
work=${FAMP_BUILD_DIR:-$HOME/famp-build}

[ -f "$bundle" ] || { echo "no such bundle: $bundle" >&2; exit 1; }
[ ! -e "$work" ] || { echo "$work already exists; move it aside or set FAMP_BUILD_DIR" >&2; exit 1; }

# `git bundle verify` needs a repository, and a fresh box has none: clone first,
# then verify the bundle from inside the clone.
git clone --quiet "$bundle" "$work"
git -C "$work" bundle verify "$bundle"
git -C "$work" checkout --quiet --detach "$tag"

got=$(git -C "$work" rev-parse "$tag^{commit}")
if [ "$got" != "$want" ]; then
  echo "REFUSING TO BUILD: tag $tag is $got, expected $want" >&2
  exit 1
fi
echo "tag $tag = $got (matches)"

cd "$work"
nice -n 19 cargo build --release --locked -j2 -p famp -p famp-gateway

for f in famp famp-gateway; do
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "target/release/$f"; else shasum -a 256 "target/release/$f"; fi
done
echo "Built in $work. To install, copy both binaries yourself, e.g.:"
echo "  install -m 0755 $work/target/release/famp $work/target/release/famp-gateway ~/.cargo/bin/"
