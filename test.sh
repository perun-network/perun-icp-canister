#!/bin/sh

export IC_COMMIT=dd3a710b03bd3ae10368a91b255571d012d1ec2f

fail()
{
	echo aborting.
	dfx stop 2>/dev/null && fg >/dev/null 2>/dev/null
	exit 1
}

cp dfx.test.json dfx.json || fail
git restore Cargo.toml

dfx stop 2>/dev/null && fg >/dev/null 2>/dev/null

dfx start &
sleep 3s
echo downloading ledger.wasm
curl -so ledger.wasm.gz https://download.dfinity.systems/ic/$IC_COMMIT/canisters/ledger-canister_notify-method.wasm.gz || fail
gunzip -f ledger.wasm.gz || fail
echo downloading ledger.private.did
curl -so ledger.private.did https://raw.githubusercontent.com/dfinity/ic/$IC_COMMIT/rs/rosetta-api/ledger.did || fail
echo downloading ledger.public.did
curl -so ledger.public.did https://raw.githubusercontent.com/dfinity/ic/$IC_COMMIT/rs/rosetta-api/ledger_canister/ledger.did || fail

dfx identity use minter 2>/dev/null || dfx identity new minter && dfx identity use minter || fail
export ICP_PERUN_MINT_ACC=`dfx ledger account-id`

dfx identity use default || fail
export ICP_PERUN_DEFAULT_ACC=`dfx ledger account-id`

dfx deploy ledger --argument '(record {minting_account = "'$ICP_PERUN_MINT_ACC'"; initial_values = vec { record { "'$ICP_PERUN_DEFAULT_ACC'"; record { e8s=0 } }; }; send_whitelist = vec {}})'
export ICP_LEDGER_PRINCIPAL=`dfx canister id ledger`
dfx deploy icp_perun
export ICP_PERUN_PRINCIPAL=`dfx canister id icp_perun`

sed -i "s/cdylib/lib/g" Cargo.toml

echo RUNNING WALKTRHOUGH
RUST_LOG=info cargo run --example happy_walkthrough

git restore Cargo.toml

dfx stop && fg >/dev/null 2>/dev/null