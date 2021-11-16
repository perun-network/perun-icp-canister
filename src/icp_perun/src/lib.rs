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
	/// Tracks the deposits per channel.
	funds: HashMap<ChannelId, Amount>,
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
		*self.funds
			.entry(funding.channel.clone())
			.or_insert(Default::default()) += amount.clone();
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

		let funds = self.funds.get(&state.state.channel).ok_or(
			Error::InsufficientFunding)?;

		self.channels.insert(
			state.state.channel.clone(),
			RegisteredState::conclude(state, &params, funds)?);
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
	assert_eq!(s.canister.conclude(s.params, sstate, 0), Err(Error::NotFinalized));
}

#[test]
/// Tests that params match the state.
fn test_conclude_invalid_params() {
	let mut s = test::Setup::new(0x23, true, true);
	let sstate = s.sign();
	s.params.challenge_duration += 1;
	assert_eq!(s.canister.conclude(s.params, sstate, 0), Err(Error::InvalidInput));
}

#[test]
/// Tests that only signed channels can be concluded.
fn test_conclude_not_signed() {
	let mut s = test::Setup::new(0xeb, true, true);
	let sstate = s.sign_invalid();
	assert_eq!(s.canister.conclude(s.params, sstate, 0), Err(Error::Authentication));
}

#[test]
/// Tests that underfunded channels cannot be concluded.
fn test_conclude_insufficient_funds() {
	let mut s = test::Setup::new(0xeb, true, true);
	s.state.allocation[0] += 1000;
	let sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params, sstate, 0), Err(Error::InsufficientFunding));
}

#[test]
/// Tests that invalid sized allocations are rejected.
fn test_conclude_invalid_allocation() {
	let mut s = test::Setup::new(0xfa, true, true);
	s.state.allocation.push(5.into());
	let signed = s.sign();
	assert_eq!(s.canister.conclude(s.params, signed, 0), Err(Error::InvalidInput));
}

#[test]
fn test_dispute_sig() {
	let mut s = test::Setup::new(0xd0, false, true);
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate), Ok(()));
}
