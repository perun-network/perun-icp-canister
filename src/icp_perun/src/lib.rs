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

use std::cell::RefCell;
use std::collections::HashMap;
pub mod error;
pub mod test;
pub mod types;

use error::*;
use ic_cdk::api::time as blocktime;
use types::*;

thread_local! {
	static STATE: RefCell<CanisterState> = Default::default();
}

#[derive(Default)]
/// The canister's state. Contains all currently registered channels, as well as
/// all deposits and withdrawable balances.
pub struct CanisterState {
	/// Tracks all deposits for unregistered channels. For registered channels,
	/// tracks withdrawable balances instead.
	holdings: HashMap<Funding, Amount>,
	/// Tracks all registered channels.
	channels: HashMap<ChannelId, RegisteredState>,
}

#[ic_cdk_macros::update]
/// Deposits funds for the specified participant into the specified channel.
/// Please do NOT over-fund or fund channels that are already fully funded, as
/// this can lead to a permanent LOSS OF FUNDS.
fn deposit(funding: Funding, amount: Amount) -> Option<Error> {
	STATE
		.with(|s| s.borrow_mut().deposit(funding, amount))
		.err()
}

#[ic_cdk_macros::update]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as settled.
fn dispute(params: Params, state: FullySignedState) -> Option<Error> {
	STATE
		.with(|s| s.borrow_mut().dispute(params, state, blocktime()))
		.err()
}

#[ic_cdk_macros::update]
/// Settles a finalized channel and makes its final funds distribution
/// withdrawable.
fn conclude(params: Params, state: FullySignedState) -> Option<Error> {
	STATE
		.with(|s| s.borrow_mut().conclude(params, state, blocktime()))
		.err()
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
		*self.holdings.entry(funding).or_insert(Default::default()) += amount;
		Ok(())
	}

	pub fn query_deposit(&self, funding: Funding) -> Option<Amount> {
		match self.holdings.get(&funding) {
			None => None,
			Some(a) => Some(a.clone()),
		}
	}

	/// Updates the holdings associated with a channel to the outcome of the
	/// supplied state, then registers the state.
	pub fn realise_outcome(&mut self, params: &Params, state: RegisteredState) {
		for (i, outcome) in state.state.allocation.iter().enumerate() {
			self.holdings.insert(
				Funding::new(state.state.channel.clone(), params.participants[i].clone()),
				outcome.clone(),
			);
		}

		self.channels.insert(state.state.channel.clone(), state);
	}

	/// Calculates the total funds held in a channel. If the channel is unknown
	/// and there are no deposited funds for the channel, returns 0.
	pub fn channel_funds(&self, channel: &ChannelId, params: &Params) -> Amount {
		let mut acc = Amount::default();
		for pk in params.participants.iter() {
			let funding = Funding::new(channel.clone(), pk.clone());
			acc += self
				.holdings
				.get(&funding)
				.unwrap_or(&Amount::default())
				.clone();
		}
		return acc;
	}

	pub fn conclude(
		&mut self,
		params: Params,
		state: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.channels.get(&state.state.channel) {
			ensure!(!old_state.settled(now), AlreadyConcluded);
		}

		let funds = &self.channel_funds(&state.state.channel, &params);

		self.realise_outcome(&params, RegisteredState::conclude(state, &params, funds)?);

		Ok(())
	}

	pub fn dispute(
		&mut self,
		params: Params,
		state: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.channels.get(&state.state.channel) {
			ensure!(!old_state.settled(now), AlreadyConcluded);
			ensure!(old_state.state.version < state.state.version, OutdatedState);
		}

		let funds = &self.channel_funds(&state.state.channel, &params);

		self.realise_outcome(
			&params,
			RegisteredState::dispute(state, &params, funds, now)?,
		);

		Ok(())
	}
}

#[test]
/// Tests that deposits are added
fn test_deposit() {
	let mut s = test::Setup::new(0xd4, false, false);

	let funding = Funding::new(s.params.id(), s.parts[0].clone());
	let funding2 = Funding::new(s.params.id(), s.parts[1].clone());
	// No deposits yet.
	assert_eq!(s.canister.query_deposit(funding.clone()), None);
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Deposit 10.
	assert_eq!(s.canister.deposit(funding.clone(), 10.into()), Ok(()));
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Now 10.
	assert_eq!(s.canister.query_deposit(funding.clone()), Some(10.into()));
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Deposit 20.
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	assert_eq!(s.canister.deposit(funding.clone(), 20.into()), Ok(()));
	// Now 30.
	assert_eq!(s.canister.query_deposit(funding), Some(30.into()));
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
}

#[test]
/// Tests the happy conclude path.
fn test_conclude() {
	let mut s = test::Setup::new(0xb2, true, true);
	let sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params, sstate, 0), Ok(()));
}

#[test]
/// Tests that nonfinal channels cannot be concluded.
fn test_conclude_nonfinal() {
	let mut s = test::Setup::new(0x1b, false, true);
	let sstate = s.sign();
	assert_eq!(
		s.canister.conclude(s.params, sstate, 0),
		Err(Error::NotFinalized)
	);
}

#[test]
/// Tests that params match the state.
fn test_conclude_invalid_params() {
	let mut s = test::Setup::new(0x23, true, true);
	let sstate = s.sign();
	s.params.challenge_duration += 1;
	assert_eq!(
		s.canister.conclude(s.params, sstate, 0),
		Err(Error::InvalidInput)
	);
}

#[test]
/// Tests that only signed channels can be concluded.
fn test_conclude_not_signed() {
	let mut s = test::Setup::new(0xeb, true, true);
	let sstate = s.sign_invalid();
	assert_eq!(
		s.canister.conclude(s.params, sstate, 0),
		Err(Error::Authentication)
	);
}

#[test]
/// Tests that underfunded channels cannot be concluded.
fn test_conclude_insufficient_funds() {
	let mut s = test::Setup::new(0xeb, true, true);
	s.state.allocation[0] += 1000;
	let sstate = s.sign();
	assert_eq!(
		s.canister.conclude(s.params, sstate, 0),
		Err(Error::InsufficientFunding)
	);
}

#[test]
/// Tests that invalid sized allocations are rejected.
fn test_conclude_invalid_allocation() {
	let mut s = test::Setup::new(0xfa, true, true);
	s.state.allocation.push(5.into());
	let signed = s.sign();
	assert_eq!(
		s.canister.conclude(s.params, signed, 0),
		Err(Error::InvalidInput)
	);
}

#[test]
fn test_dispute_nonfinal() {
	let mut s = test::Setup::new(0xd0, false, true);
	let now = 0;
	let channel = s.params.id();
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate, now), Ok(()));
	assert!(!s.canister.channels.get(&channel).unwrap().settled(now));
}

#[test]
fn test_dispute_final() {
	let time = 0;
	let mut s = test::Setup::new(0xd0, true, true);
	let channel = s.params.id();
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate, time), Ok(()));
	assert!(s.canister.channels.get(&channel).unwrap().settled(time));
}

#[test]
fn test_dispute_valid_refutation() {
	let time = 0;
	let mut s = test::Setup::new(0xbf, false, true);
	let channel = s.params.id();
	let mut sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params.clone(), sstate, time), Ok(()));
	s.state.version += 1;
	s.state.finalized = true;
	sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate, time), Ok(()));
	assert!(s.canister.channels.get(&channel).unwrap().settled(time));
}

#[test]
fn test_dispute_outdated_refutation() {
	let time = 0;
	let version = 10;
	let mut s = test::Setup::new(0x21, false, true);
	let channel = s.params.id();
	s.state.version = version;
	let mut sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params.clone(), sstate, time), Ok(()));
	s.state.version -= 1;
	sstate = s.sign();
	assert_eq!(
		s.canister.dispute(s.params, sstate, time),
		Err(Error::OutdatedState)
	);
	assert!(!s.canister.channels.get(&channel).unwrap().settled(time));
	assert_eq!(
		s.canister.channels.get(&channel).unwrap().state.version,
		version
	);
}

#[test]
fn test_dispute_settled_refutation() {
	let time = 0;
	let version = 10;
	let mut s = test::Setup::new(0x21, true, true);
	let channel = s.params.id();
	s.state.version = version;
	let mut sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params.clone(), sstate, time), Ok(()));
	s.state.version += 1;
	sstate = s.sign();
	assert_eq!(
		s.canister.dispute(s.params, sstate, time),
		Err(Error::AlreadyConcluded)
	);
	assert!(s.canister.channels.get(&channel).unwrap().settled(time));
	assert_eq!(
		s.canister.channels.get(&channel).unwrap().state.version,
		version
	);
}

#[test]
fn test_holding_tracking_deposit() {
	let s = test::Setup::new(0xd9, true, true);
	let sum = s.state.allocation[0].clone() + s.state.allocation[1].clone();
	assert_eq!(s.canister.channel_funds(&s.state.channel, &s.params), sum);
}

#[test]
fn test_holding_tracking_none() {
	let s = test::Setup::new(0xd9, true, false);
	assert_eq!(s.canister.channel_funds(&s.state.channel, &s.params), 0);
}
