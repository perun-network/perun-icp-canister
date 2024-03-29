#!/usr/bin/env bash
set -e

die() {
	echo "$1" >&2
	exit 1
}

cargo --version >/dev/null || die "Must have cargo installed."

export RUSTFLAGS="--remap-path-prefix=\"${PWD}\"=./ --remap-path-prefix=\"${HOME}\"=_/"
cargo build --release --target wasm32-unknown-unknown

echo "Installing ic-cdk-optimizer…"
if cargo install ic-cdk-optimizer --root target -q; then
	target/bin/ic-cdk-optimizer \
		target/wasm32-unknown-unknown/release/icp_perun.wasm \
		-o target/wasm32-unknown-unknown/release/icp_perun-opt.wasm
else
	die "Could not install ic-cdk-optimizer (see above)."
fi
