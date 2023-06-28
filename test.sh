#!/bin/sh

fail()
{
	echo aborting.
	dfx stop 2>/dev/null && fg >/dev/null 2>/dev/null
	exit 1
}

dfx stop 2>/dev/null && fg >/dev/null 2>/dev/null

dfx start --clean &
sleep 3s

dfx identity use minter 2>/dev/null || fail
#export ICP_PERUN_MINT_ACC=`dfx ledger account-id`
mintaccount_id="f45c4e592c1560c737ee4c99b17ad34f27f2e6a9925955742c820770eecbf414"
icpaccount_id="e1ee72e36807cf24b1e56df82224c8991af50797e1fde26588aeb3306491c5bd"
export ICP_PERUN_MINT_ACC="$mintaccount_id"
export ICP_PERUN_DEFAULT_ACC="$icpaccount_id"

echo "Defined env variables"

dfx identity use minter || fail
#export ICP_PERUN_DEFAULT_ACC=`dfx ledger account-id`

echo "Deploying ledger and icp_perun"

dfx deploy ledger --argument '(record {minting_account = "'${ICP_PERUN_MINT_ACC}'"; initial_values = vec { record { "'${ICP_PERUN_DEFAULT_ACC}'"; record { e8s=0 } }; }; send_whitelist = vec {}})'
echo "Deployed ledger"

export ICP_LEDGER_PRINCIPAL="bkyz2-fmaaa-aaaaa-qaaaq-cai"
#`dfx canister id ledger`
dfx deploy icp_perun
export ICP_PERUN_PRINCIPAL="be2us-64aaa-aaaaa-qaabq-cai"
#`dfx canister id icp_perun`

sed -i "s/cdylib/lib/g" Cargo.toml

echo "Deploying ICP_PERUN"


echo RUNNING WALKTRHOUGH

RUST_LOG=info cargo run --example happy_walkthrough

dfx stop && fg >/dev/null 2>/dev/null