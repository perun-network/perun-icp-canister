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

use std::collections::HashMap;
use std::cell::RefCell;
pub mod error;
pub mod types;
pub mod test;

use types::*;
use error::*;
use ic_cdk::api::time as blocktime;
use candid::Encode;

thread_local! {
	static STATE: RefCell<CanisterState> = Default::default();
}

#[derive(Default)]
/// The canister's state. Contains all currently registered channels, as well as
/// all deposits and withdrawable balances.
pub struct CanisterState {
	/// Tracks all deposits for unregistered channels. For registered channels,
	/// tracks withdrawable balances instead.
	deposits: HashMap<Funding, Amount>,
	/// Tracks all registered channels.
	channels: HashMap<ChannelId, RegisteredState>,
}

#[ic_cdk_macros::update]
/// Deposits funds for the specified participant into the specified channel.
/// Please do NOT over-fund or fund channels that are already fully funded, as
/// this can lead to a permanent LOSS OF FUNDS.
fn deposit(funding: Funding, amount: Amount) -> Option<Error> {
	STATE.with(|s| s.borrow_mut().deposit(funding, amount)).err()
}

#[ic_cdk_macros::update]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as settled.
fn dispute(params: Params, state: FullySignedState) -> Option<Error> {
	STATE.with(|s| s.borrow_mut().dispute(params, state)).err()
}

#[ic_cdk_macros::update]
/// Settles a finalized channel and makes its final funds distribution
/// withdrawable.
fn conclude(params: Params, state: FullySignedState) -> Option<Error> {
	STATE.with(|s| s.borrow_mut().conclude(params, state, blocktime())).err()
}

#[ic_cdk_macros::update]
/// Withdraws the specified participant's funds from a settled channel.
fn withdraw(_request: WithdrawalRequest, _auth: L2Signature) -> () {}

#[ic_cdk_macros::query]
/// Returns the funds deposited for a channel's specified participant, if any.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_deposit(funding: Funding) -> Option<Amount> {
	STATE.with(|s| s.borrow().query_deposit(funding))
}

impl CanisterState {
	pub fn deposit(&mut self, funding: Funding, amount: Amount) -> Result<()> {
		*self.deposits
			.entry(funding)
			.or_insert(Default::default()) += amount;
		Ok(())
	}

	pub fn query_deposit(&self, funding: Funding) -> Option<Amount> {
		match self.deposits.get(&funding) {
			None => None,
			Some(a) => Some(a.clone()),
		}
	}

	pub fn conclude(&mut self, params: Params, state: FullySignedState, now: Timestamp) -> Result<()> {
		if let Some(old_state) = self.channels.get(&state.state.channel) {
			if old_state.settled(now) {
				Err(Error::AlreadyConcluded)?;
			}
		}

		self.channels.insert(
			state.state.channel.clone(),
			RegisteredState::conclude(state, &params)?);
		Ok(())
	}

	pub fn dispute(&mut self, params: Params, state: FullySignedState) -> Result<()> {
		for (i, pk) in params.participants.iter().enumerate() {
			state.state.validate_sig(&state.sigs[i], &pk)?;
		}
		Ok(())
	}
}


#[test]
/// Tests that deposits are added 
fn test_deposit() {
	let channel = ChannelId::default();
	let (mut canister, _, pk) = test::setup();
	let funding = Funding::new(channel.clone(), pk);
	let funding2 = Funding::new(channel, L2Account::default());
	// No deposits yet.
	assert_eq!(canister.query_deposit(funding.clone()), None);
	assert_eq!(canister.query_deposit(funding2.clone()), None);
	// Deposit 10.
	assert_eq!(canister.deposit(funding.clone(), 10.into()), Ok(()));
	assert_eq!(canister.query_deposit(funding2.clone()), None);
	// Now 10.
	assert_eq!(canister.query_deposit(funding.clone()), Some(10.into()));
	assert_eq!(canister.query_deposit(funding2.clone()), None);
	// Deposit 20.
	assert_eq!(canister.query_deposit(funding2.clone()), None);
	assert_eq!(canister.deposit(funding.clone(), 20.into()), Ok(()));
	// Now 30.
	assert_eq!(canister.query_deposit(funding), Some(30.into()));
	assert_eq!(canister.query_deposit(funding2.clone()), None);
}

#[test]
/// Tests the happy conclude path.
fn test_conclude() {
	let (mut canister, sk, pk) = test::setup();
	let mut params = Params::default();
	params.participants = vec![pk.clone()];
	let mut state = State::default();
	state.channel = params.id();
	state.version = 1;
	state.allocation = vec![10.into()];
	state.finalized = true;

	let enc = Encode!(&state).unwrap();
	let mut signed = FullySignedState::default();
	signed.state = state;
	signed.sigs = vec![L2Signature(sk.sign(&enc, &pk.0).to_bytes().into())];

	assert_eq!(canister.conclude(params, signed, 0), Ok(()));
}

#[test]
/// Tests that nonfinal channels cannot be concluded.
fn test_conclude_nonfinal() {
	let (mut canister, sk, pk) = test::setup();
	let mut params = Params::default();
	params.participants = vec![pk.clone()];
	let mut state = State::default();
	state.channel = params.id();
	state.version = 1;
	state.allocation = vec![10.into()];
	state.finalized = false;

	let enc = Encode!(&state).unwrap();
	let mut signed = FullySignedState::default();
	signed.state = state;
	signed.sigs = vec![L2Signature(sk.sign(&enc, &pk.0).to_bytes().into())];

	assert_eq!(canister.conclude(params, signed, 0), Err(Error::NotFinalized));
}

#[test]
/// Tests that params match the state.
fn test_conclude_invalid_params() {
	let (mut canister, sk, pk) = test::setup();
	let mut params = Params::default();
	params.participants = vec![pk.clone()];
	let mut state = State::default();
	state.channel = params.id();
	params.nonce = vec![1];
	state.version = 1;
	state.allocation = vec![10.into()];
	state.finalized = true;

	let enc = Encode!(&state).unwrap();
	let mut signed = FullySignedState::default();
	signed.state = state;
	signed.sigs = vec![L2Signature(sk.sign(&enc, &pk.0).to_bytes().into())];

	assert_eq!(canister.conclude(params, signed, 0), Err(Error::InvalidInput));
}

#[test]
/// Tests that only signed channels can be concluded.
fn test_conclude_not_signed() {
	let (mut canister, sk, pk) = test::setup();
	let mut params = Params::default();
	params.participants = vec![pk.clone()];
	let mut state = State::default();
	state.channel = params.id();
	state.version = 1;
	state.allocation = vec![10.into()];
	state.finalized = true;

	let enc = Encode!(&"invalid state").unwrap();
	let mut signed = FullySignedState::default();
	signed.state = state;
	signed.sigs = vec![L2Signature(sk.sign(&enc, &pk.0).to_bytes().into())];

	assert_eq!(canister.conclude(params, signed, 0), Err(Error::Authentication));
}

#[test]
/// Tests that invalid sized allocations are rejected.
fn test_conclude_invalid_allocation() {
	let (mut canister, sk, pk) = test::setup();
	let mut params = Params::default();
	params.participants = vec![pk.clone()];
	let mut state = State::default();
	state.channel = params.id();
	state.version = 1;
	state.finalized = true;

	let enc = Encode!(&state).unwrap();
	let mut signed = FullySignedState::default();
	signed.state = state;
	signed.sigs = vec![L2Signature(sk.sign(&enc, &pk.0).to_bytes().into())];

	assert_eq!(canister.conclude(params, signed, 0), Err(Error::InvalidInput));
}

#[test]
fn test_dispute_sig() {
	let (mut canister, alice_esk, alice) = test::setup();

	let hash = vec![123u8; 32];
	let state = State {
		channel: hash.clone(),
		version: 564,
		allocation: vec![10.into()],
		finalized: false,
	};
	let enc = Encode!(&state).unwrap();
	let alice_sig = L2Signature(alice_esk.sign(&enc, &alice.0).to_bytes().into());

	let sstate = FullySignedState {
		state: state,
		sigs: vec![alice_sig],
	};
	let params = Params {
		nonce: hash,
		participants: vec![alice],
		challenge_duration: 123,
	};
	assert_eq!(canister.dispute(params, sstate), Ok(()));
}
