#!/bin/sh

fail()
{
	echo aborting.
	dfx stop 2>/dev/null && fg >/dev/null 2>/dev/null
	exit 1
}

export ICP_LEDGER_PRINCIPAL="bkyz2-fmaaa-aaaaa-qaaaq-cai"
#`dfx canister id ledger`

export ICP_PERUN_PRINCIPAL="be2us-64aaa-aaaaa-qaabq-cai"
#`dfx canister id icp_perun`

sed -i "s/cdylib/lib/g" Cargo.toml


echo RUNNING WALKTRHOUGH

RUST_LOG=info cargo run --example happy_walkthrough

dfx stop && fg >/dev/null 2>/dev/null