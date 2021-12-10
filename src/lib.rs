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

pub mod error;
pub mod types;
// We don't need testing code in wasm output, only for tests and examples
#[cfg(not(target_family = "wasm"))]
pub mod test;
// The actual canister tests
#[cfg(test)]
mod tests;

use ic_cdk::api::time as blocktime;
use std::cell::RefCell;
use std::collections::HashMap;

use error::*;
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
fn query_holdings(funding: Funding) -> Option<Amount> {
	STATE.with(|s| s.borrow().query_holdings(funding))
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

	pub fn query_holdings(&self, funding: Funding) -> Option<Amount> {
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
		let total = &self.holdings_total(&params);
		if total < &state.state.total() {
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
	pub fn holdings_total(&self, params: &Params) -> Amount {
		let mut acc = Amount::default();
		for pk in params.participants.iter() {
			let funding = Funding::new(params.id(), pk.clone());
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
