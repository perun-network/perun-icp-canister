//  Copyright 2021 PolyCrypt GmbH
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

use crate::*;
use assert::assert_ok;

#[test]
fn save_candid() {
	use super::export_candid;
	use std::env;
	use std::fs::{create_dir_all, write};
	use std::path::PathBuf;

	let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
	create_dir_all(&dir).expect("Failed to create directory.");

	write(dir.join("aaaa.did"), export_candid()).expect("Write failed.");
}

#[test]
/// Tests that repeated deposits are added correctly and that only the specified
/// participant is credited. Also tests the `query_holdings()` method.
fn test_deposit() {
	let mut s = test::Setup::new(false, false);

	let funding = Funding::new(s.params.id(), s.parts[0].clone());
	let funding2 = Funding::new(s.params.id(), s.parts[1].clone());
	// No deposits yet.
	assert_eq!(s.canister.query_holdings(funding.clone()), None);
	assert_eq!(s.canister.query_holdings(funding2.clone()), None);
	// Deposit 10.
	assert_ok!(s.canister.deposit(funding.clone(), 10.into()));
	// Now 10.
	assert_eq!(s.canister.query_holdings(funding.clone()), Some(10.into()));
	assert_eq!(s.canister.query_holdings(funding2.clone()), None);
	// Deposit 20.
	assert_eq!(s.canister.query_holdings(funding2.clone()), None);
	assert_ok!(s.canister.deposit(funding.clone(), 20.into()));
	// Now 30.
	assert_eq!(s.canister.query_holdings(funding.clone()), Some(30.into()));
	assert_eq!(s.canister.query_holdings(funding2.clone()), None);
	// Deposit 45 to second party.
	assert_ok!(s.canister.deposit(funding2.clone(), 45.into()));
	assert_eq!(s.canister.query_holdings(funding), Some(30.into()));
	assert_eq!(s.canister.query_holdings(funding2), Some(45.into()));
}

#[test]
/// Tests the happy conclude path using a final state.
fn test_conclude() {
	let mut s = test::Setup::new(true, true);
	let sstate = s.sign_state();
	assert_ok!(s.canister.conclude_can(s.params, sstate, 0));
}

#[test]
/// Tests that nonfinal channels cannot be concluded.
fn test_conclude_nonfinal() {
	let mut s = test::Setup::new(false, true);
	let sstate = s.sign_state();
	assert_eq!(
		s.canister.conclude_can(s.params, sstate, 0),
		Err(Error::NotFinalized)
	);
}

#[test]
/// Tests that the supplied params must match the state.
fn test_conclude_invalid_params() {
	let mut s = test::Setup::new(true, true);
	let sstate = s.sign_state();
	s.params.challenge_duration += 1;
	assert_eq!(
		s.canister.conclude_can(s.params, sstate, 0),
		Err(Error::InvalidInput)
	);
}

#[test]
/// Tests that only signed channels can be concluded.
fn test_conclude_not_signed() {
	let mut s = test::Setup::new(true, true);
	let sstate = s.sign_state_invalid();
	assert_eq!(
		s.canister.conclude_can(s.params, sstate, 0),
		Err(Error::Authentication)
	);
}

#[test]
/// Tests that underfunded channels cannot be concluded.
fn test_conclude_insufficient_funds() {
	let mut s = test::Setup::new(true, true);
	s.state.allocation[0] += 1000;
	let sstate = s.sign_state();
	assert_eq!(
		s.canister.conclude_can(s.params, sstate, 0),
		Err(Error::InsufficientFunding)
	);
}

#[test]
/// Tests that invalid sized allocations are rejected.
fn test_conclude_invalid_allocation() {
	let mut s = test::Setup::new(true, true);
	s.state.allocation.push(5.into());
	let signed = s.sign_state();
	assert_eq!(
		s.canister.conclude_can(s.params, signed, 0),
		Err(Error::InvalidInput)
	);
}

#[test]
/// Tests that a dispute with a nonfinal state will register the state properly
/// but not mark it as final yet.
fn test_dispute_nonfinal() {
	let mut s = test::Setup::new(false, true);
	let now = 0;
	let channel = s.params.id();
	let sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params, sstate, now));
	assert!(!s.canister.state(&channel).unwrap().settled(now));
}

#[test]
/// Tests that dispute with a final state will register the state and mark it as
/// final.
fn test_dispute_final() {
	let time = 0;
	let mut s = test::Setup::new(true, true);
	let channel = s.params.id();
	let sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params, sstate, time));
	assert!(s.canister.state(&channel).unwrap().settled(time));
}

#[test]
/// Tests that a newer channel state can replace an older channel state if it is
/// not yet final.
fn test_dispute_valid_refutation() {
	let time = 0;
	let mut s = test::Setup::new(false, true);
	let channel = s.params.id();
	let mut sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params.clone(), sstate, time));
	s.state.version += 1;
	s.state.finalized = true;
	sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params, sstate, time));
	assert!(s.canister.state(&channel).unwrap().settled(time));
}

#[test]
/// Tests that a refutation using an older state fails.
fn test_dispute_outdated_refutation() {
	let time = 0;
	let version = 10;
	let mut s = test::Setup::new(false, true);
	let channel = s.params.id();
	s.state.version = version;
	let mut sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params.clone(), sstate, time));
	s.state.version -= 1;
	sstate = s.sign_state();
	assert_eq!(
		s.canister.dispute_can(s.params, sstate, time),
		Err(Error::OutdatedState)
	);
	assert!(!s.canister.state(&channel).unwrap().settled(time));
	assert_eq!(s.canister.state(&channel).unwrap().state.version, version);
}

#[test]
/// Tests that a settled state cannot be refuted.
fn test_dispute_settled_refutation() {
	let time = 0;
	let version = 10;
	let mut s = test::Setup::new(true, true);
	let channel = s.params.id();
	s.state.version = version;
	let mut sstate = s.sign_state();
	assert_ok!(s.canister.conclude_can(s.params.clone(), sstate, time));
	s.state.version += 1;
	sstate = s.sign_state();
	assert_eq!(
		s.canister.dispute_can(s.params, sstate, time),
		Err(Error::AlreadyConcluded)
	);
	assert!(s.canister.state(&channel).unwrap().settled(time));
	assert_eq!(s.canister.state(&channel).unwrap().state.version, version);
}

#[test]
/// Tests that the initial state of a channel in a dispute may be under-funded,
/// but other states must not be.
fn test_dispute_underfunded_initial_state() {
	let mut time = 0;
	let mut s = test::Setup::new(false, false);

	let amount = s.state.allocation[0].clone();
	// only fund one participant.
	assert_ok!(s.canister.deposit(s.funding(0), amount.clone()));

	s.state.version = 0;
	assert_eq!(
		s.canister
			.dispute_can(s.params.clone(), s.sign_state(), time),
		Ok(())
	);
	s.state.version = 1;
	assert_eq!(
		s.canister
			.dispute_can(s.params.clone(), s.sign_state(), time),
		Err(Error::InsufficientFunding)
	);

	// Wait for the channel to be finalised.
	time += &s.params.challenge_duration;
	assert!(s
		.canister
		.channels
		.get(&s.params.id())
		.unwrap()
		.settled(time));

	// Withdraw the funding.
	let (req, sig) = s.withdrawal(0);
	assert_eq!(s.canister.withdraw_can(req, sig, time), Ok(amount.clone()));

	let (req, sig) = s.withdrawal(1);
	assert_eq!(
		s.canister.withdraw_can(req, sig, time),
		Ok(Amount::default())
	);
}

#[test]
/// Tests that the total deposits are properly tracked.
fn test_holding_tracking_deposit() {
	let s = test::Setup::new(true, true);
	let sum = s.state.allocation[0].clone() + s.state.allocation[1].clone();
	assert_eq!(s.canister.holdings_total(&s.params), sum);
}

#[test]
/// Tests that unregistered channels are counted as unfunded.
fn test_holding_tracking_none() {
	let s = test::Setup::new(true, false);
	assert_eq!(s.canister.holdings_total(&s.params), 0);
}

#[test]
/// Tests the happy case for withdrawing funds from a settled channel. Also
/// tests that redundant withdrawals will not withdraw any additional funds.
fn test_withdraw() {
	let mut s = test::Setup::new(true, true);
	let sstate = s.sign_state();
	assert_ok!(s.canister.conclude_can(s.params.clone(), sstate, 0));

	let (req, sig) = s.withdrawal(0);

	let holdings = s.canister.query_holdings(s.funding(0)).unwrap();
	assert_eq!(
		s.canister.withdraw_can(req.clone(), sig.clone(), 0),
		Ok(holdings)
	);

	// Test that repeated withdraws return nothing.
	assert_eq!(s.canister.withdraw_can(req, sig, 0), Ok(Amount::default()));
}

#[test]
/// Tests that the signature of withdrawal requests must be valid.
fn test_withdraw_invalid_sig() {
	let mut s = test::Setup::new(true, true);
	let sstate = s.sign_state();
	assert_ok!(s.canister.conclude_can(s.params.clone(), sstate, 0));

	let (req, _) = s.withdrawal(0);
	let sig = s.sign_withdrawal(&req, 1); // sign with wrong user.

	assert_eq!(
		s.canister.withdraw_can(req, sig, 0),
		Err(Error::Authentication)
	);
}

#[test]
/// Tests that the channel to be withdrawn from must be known.
fn test_withdraw_unknown_channel() {
	let mut s = test::Setup::new(true, true);
	//let unknown_id = test::rand_hash(&mut s.prng);
	let sstate = s.sign_state();
	assert_ok!(s.canister.conclude_can(s.params.clone(), sstate, 0));

	let (mut req, _) = s.withdrawal(0);
	let unknown_hash = test::rand_hash(&mut s.prng);
	let unknown_id = hash_to_channel_id(&unknown_hash);
	req.funding.channel = unknown_id;

	let sig = s.sign_withdrawal(&req, 0);

	assert_eq!(
		s.canister.withdraw_can(req, sig, 0),
		Err(Error::NotFinalized)
	);
}

#[test]
/// Tests that the channel to be withdrawn from must be settled.
fn test_withdraw_not_finalized() {
	let mut s = test::Setup::new(false, true);
	let now = 0;
	let sstate = s.sign_state();
	assert_ok!(s.canister.dispute_can(s.params.clone(), sstate, now));
	assert!(!s
		.canister
		.channels
		.get(&s.params.id())
		.unwrap()
		.settled(now));

	let (req, sig) = s.withdrawal(0);

	assert_eq!(
		s.canister.withdraw_can(req, sig, 0),
		Err(Error::NotFinalized)
	);
}
