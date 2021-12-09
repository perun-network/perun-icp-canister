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
fn withdraw(request: WithdrawalRequest, auth: L2Signature) -> (Option<Amount>, Option<Error>) {
	let result = STATE.with(|s| s.borrow_mut().withdraw(request, auth, blocktime()));
	(result.as_ref().ok().cloned(), result.err())
}

#[ic_cdk_macros::query]
/// Returns the funds deposited for a channel's specified participant, if any.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_deposit(funding: Funding) -> Option<Amount> {
	STATE.with(|s| s.borrow().query_deposit(funding))
}

#[ic_cdk_macros::query]
/// Returns the latest registered state for a given channel and its dispute
/// timeout. This function should be used to check for registered disputes.
fn query_state(id: ChannelId) -> Option<RegisteredState> {
	STATE.with(|s| s.borrow().state(&id))
}

impl CanisterState {
	pub fn deposit(&mut self, funding: Funding, amount: Amount) -> Result<()> {
		*self.holdings.entry(funding).or_insert(Default::default()) += amount;
		Ok(())
	}

	pub fn query_deposit(&self, funding: Funding) -> Option<Amount> {
		self.holdings.get(&funding).cloned()
	}

	/// Queries a registered state.
	pub fn state(&self, id: &ChannelId) -> Option<RegisteredState> {
		self.channels.get(&id).cloned()
	}

	/// Updates the holdings associated with a channel to the outcome of the
	/// supplied state, then registers the state. If the state is the channel's
	/// initial state, the holdings are not updated, as initial states are
	/// allowed to be under-funded and are otherwise expected to match the
	/// deposit distribution exactly if fully funded.
	fn register_channel(&mut self, params: &Params, state: RegisteredState) -> Result<()> {
		let deposits = &self.channel_funds(&state.state.channel, &params);
		if deposits < &state.state.total() {
			require!(state.state.may_be_underfunded(), InsufficientFunding);
		} else {
			self.update_holdings(&params, &state.state);
		}

		self.channels.insert(state.state.channel.clone(), state);
		Ok(())
	}

	/// Pushes a state's funding allocation into the channel's holdings mapping
	/// in the canister.
	fn update_holdings(&mut self, params: &Params, state: &State) {
		for (i, outcome) in state.allocation.iter().enumerate() {
			self.holdings.insert(
				Funding::new(state.channel.clone(), params.participants[i].clone()),
				outcome.clone(),
			);
		}
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
		acc
	}

	pub fn conclude(
		&mut self,
		params: Params,
		state: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.state(&state.state.channel) {
			require!(!old_state.settled(now), AlreadyConcluded);
		}

		self.register_channel(&params, RegisteredState::conclude(state, &params)?)
	}

	pub fn dispute(
		&mut self,
		params: Params,
		state: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.state(&state.state.channel) {
			require!(!old_state.settled(now), AlreadyConcluded);
			require!(old_state.state.version < state.state.version, OutdatedState);
		}

		self.register_channel(&params, RegisteredState::dispute(state, &params, now)?)
	}

	pub fn withdraw(
		&mut self,
		req: WithdrawalRequest,
		auth: L2Signature,
		now: Timestamp,
	) -> Result<Amount> {
		req.validate_sig(&auth)?;
		match self.state(&req.funding.channel) {
			None => Err(Error::NotFinalized),
			Some(state) => {
				require!(state.settled(now), NotFinalized);
				Ok(self.holdings.remove(&req.funding).unwrap_or_default())
			}
		}
	}
}

#[test]
/// Tests that repeated deposits are added correctly and that only the specified
/// participant is credited. Also tests the `query_deposit()` method.
fn test_deposit() {
	let mut s = test::Setup::new(0xd4, false, false);

	let funding = Funding::new(s.params.id(), s.parts[0].clone());
	let funding2 = Funding::new(s.params.id(), s.parts[1].clone());
	// No deposits yet.
	assert_eq!(s.canister.query_deposit(funding.clone()), None);
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Deposit 10.
	assert_eq!(s.canister.deposit(funding.clone(), 10.into()), Ok(()));
	// Now 10.
	assert_eq!(s.canister.query_deposit(funding.clone()), Some(10.into()));
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Deposit 20.
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	assert_eq!(s.canister.deposit(funding.clone(), 20.into()), Ok(()));
	// Now 30.
	assert_eq!(s.canister.query_deposit(funding.clone()), Some(30.into()));
	assert_eq!(s.canister.query_deposit(funding2.clone()), None);
	// Deposit 45 to second party.
	assert_eq!(s.canister.deposit(funding2.clone(), 45.into()), Ok(()));
	assert_eq!(s.canister.query_deposit(funding), Some(30.into()));
	assert_eq!(s.canister.query_deposit(funding2), Some(45.into()));
}

#[test]
/// Tests the happy conclude path using a final state.
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
/// Tests that the supplied params must match the state.
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
/// Tests that a dispute with a nonfinal state will register the state properly
/// but not mark it as final yet.
fn test_dispute_nonfinal() {
	let mut s = test::Setup::new(0xd0, false, true);
	let now = 0;
	let channel = s.params.id();
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate, now), Ok(()));
	assert!(!s.canister.state(&channel).unwrap().settled(now));
}

#[test]
/// Tests that dispute with a final state will register the state and mark it as
/// final.
fn test_dispute_final() {
	let time = 0;
	let mut s = test::Setup::new(0xd0, true, true);
	let channel = s.params.id();
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params, sstate, time), Ok(()));
	assert!(s.canister.state(&channel).unwrap().settled(time));
}

#[test]
/// Tests that a newer channel state can replace an older channel state if it is
/// not yet final.
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
	assert!(s.canister.state(&channel).unwrap().settled(time));
}

#[test]
/// Tests that a refutation using an older state fails.
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
	assert!(!s.canister.state(&channel).unwrap().settled(time));
	assert_eq!(s.canister.state(&channel).unwrap().state.version, version);
}

#[test]
/// Tests that a settled state cannot be refuted.
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
	assert!(s.canister.state(&channel).unwrap().settled(time));
	assert_eq!(s.canister.state(&channel).unwrap().state.version, version);
}

#[test]
/// Tests that the initial state of a channel in a dispute may be under-funded,
/// but other states must not be.
fn test_dispute_underfunded_initial_state() {
	let mut time = 0;
	let mut s = test::Setup::new(0x95, false, false);

	let amount = s.state.allocation[0].clone();
	// only fund one participant.
	assert_eq!(s.canister.deposit(s.funding(0), amount.clone()), Ok(()));

	s.state.version = 0;
	assert_eq!(s.canister.dispute(s.params.clone(), s.sign(), time), Ok(()));
	s.state.version = 1;
	assert_eq!(
		s.canister.dispute(s.params.clone(), s.sign(), time),
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
	let (req, sig) = s.withdrawal(0, test::default_account());
	assert_eq!(s.canister.withdraw(req, sig, time), Ok(amount.clone()));
}

#[test]
/// Tests that the total deposits are properly tracked.
fn test_holding_tracking_deposit() {
	let s = test::Setup::new(0xd9, true, true);
	let sum = s.state.allocation[0].clone() + s.state.allocation[1].clone();
	assert_eq!(s.canister.channel_funds(&s.state.channel, &s.params), sum);
}

#[test]
/// Tests that unregistered channels are counted as unfunded.
fn test_holding_tracking_none() {
	let s = test::Setup::new(0xd9, true, false);
	assert_eq!(s.canister.channel_funds(&s.state.channel, &s.params), 0);
}

#[test]
/// Tests the happy case for withdrawing funds from a settled channel. Also
/// tests that redundant withdrawals will not withdraw any additional funds.
fn test_withdraw() {
	let mut s = test::Setup::new(0xab, true, true);
	let sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params.clone(), sstate, 0), Ok(()));

	let (req, sig) = s.withdrawal(0, test::default_account());

	let holdings = s.canister.query_deposit(s.funding(0)).unwrap();
	assert_eq!(
		s.canister.withdraw(req.clone(), sig.clone(), 0),
		Ok(holdings)
	);

	// Test that repeated withdraws return nothing.
	assert_eq!(s.canister.withdraw(req, sig, 0), Ok(Amount::default()));
}

#[test]
/// Tests that the signature of withdrawal requests must be valid.
fn test_withdraw_invalid_sig() {
	let mut s = test::Setup::new(0x28, true, true);
	let sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params.clone(), sstate, 0), Ok(()));

	let (req, _) = s.withdrawal(0, test::default_account());
	let sig = s.sign_withdrawal(&req, 1); // sign with wrong user.

	assert_eq!(s.canister.withdraw(req, sig, 0), Err(Error::Authentication));
}

#[test]
/// Tests that the channel to be withdrawn from must be known.
fn test_withdraw_unknown_channel() {
	let rand = 0x53;
	let mut s = test::Setup::new(rand, true, true);
	let unknown_id = test::Setup::new(rand + 1, false, false).params.id();
	let sstate = s.sign();
	assert_eq!(s.canister.conclude(s.params.clone(), sstate, 0), Ok(()));

	let (mut req, _) = s.withdrawal(0, test::default_account());
	req.funding.channel = unknown_id;

	let sig = s.sign_withdrawal(&req, 0);

	assert_eq!(s.canister.withdraw(req, sig, 0), Err(Error::NotFinalized));
}

#[test]
/// Tests that the channel to be withdrawn from must be settled.
fn test_withdraw_not_finalized() {
	let mut s = test::Setup::new(0x59, false, true);
	let now = 0;
	let sstate = s.sign();
	assert_eq!(s.canister.dispute(s.params.clone(), sstate, now), Ok(()));
	assert!(!s
		.canister
		.channels
		.get(&s.params.id())
		.unwrap()
		.settled(now));

	let (req, sig) = s.withdrawal(0, test::default_account());

	assert_eq!(s.canister.withdraw(req, sig, 0), Err(Error::NotFinalized));
}
