#!/usr/bin/env bash
set -e

die() {
	echo "$1" >&2
	exit 1
}

# Ensure `cargo` and `ic-wasm` are installed.
cargo --version >/dev/null || die "Must have cargo installed."

# Build with cargo.
export RUSTFLAGS="--remap-path-prefix=\"${PWD}\"=./ --remap-path-prefix=\"${HOME}\"=_/"
cargo build --release --target wasm32-unknown-unknown

# Use ic-wasm to shrink the Wasm binary.
echo "Shrinking with ic-wasm..."
if cargo install ic-wasm --root target -q; then
    target/bin/ic-wasm \
        target/wasm32-unknown-unknown/release/icp_perun.wasm \
        -o target/wasm32-unknown-unknown/release/icp_perun-opt.wasm \
        shrink
else
    die "Could not shrink the Wasm binary with ic-wasm (see above)."
fi