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

use std::collections::HashMap as Map;
pub mod types;
pub mod error;
use types::*;
use error::*;

use ic_cdk::api::{caller, time};

#[ic_cdk_macros::update]
fn deposit(funding: Funding, amount: Amount) {
	contract.deposit(funding, amount)
}

#[ic_cdk_macros::update]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as finalized.
fn dispute(state: FullySignedState) -> () {
	ic_cdk::print("disputing");
}

#[ic_cdk_macros::update]
/// Settles a finalized channel. It can then be used for withdrawing.
fn settle(state: FullySignedState, params: Params) -> () {
	contract.settle(state, params);
}

#[ic_cdk_macros::update]
/// Withdraws funds from a settled channel.
fn withdraw(funding: Funding, auth: L2Signature) -> () {
	let request = WithdrawalRequest::new(funding, caller());
	contract.withdraw(request, auth, time())
}

#[ic_cdk_macros::query]
fn query_deposit(funding: Funding) -> Amount {
}

/// contract contains the contract's state storage.
static contract: Perun = Perun::new();


/// Perun contains all state and logic for the Perun channel framework.
pub struct Perun {
	pub deposits: Map<Funding, Amount>,
	pub channels: Map<ChannelId, RegisteredState>,
	pub clearFunds: Map<Funding, Amount>,
}

impl Perun {
	pub fn new() -> Self {
		Self {
			deposits: Map::new(),
			channels: Map::new(),
			clearFunds: Map::new(),
		}
	}

	pub fn deposit(self: &mut Self, funding: Funding, amount: Amount) {
		self.deposits.insert(funding, amount);
	}

	pub fn settle(self: &mut Self, state: FullySignedState, params: Params) -> Result<()> {
		if let Err(_) = state.verify(&params) {
			Err(Error::InvalidSignatures)?;
		}

		self.settle_auth(&state.state, &params)
	}

	pub fn settle_auth(self: &mut Self, state: &State, params: &Params) -> Result<()> {
		if !state.finalized {
			Err(Error::StateNotFinal)?;
		}

		if !params.matches(&state) {
			Err(Error::MalformedInput)?;
		}

		self.channels.insert(state.channel, RegisteredState::settled(state));

		for (i, acc) in params.participants.into_iter().enumerate() {
			let f = Funding::new(state.channel, acc);
			self.clearFunds.insert(f, state.allocation[i]);
		}

		Ok(())
	}

	pub fn withdraw(self: &mut Self, req: WithdrawalRequest, auth: L2Signature, time: Timestamp) -> Result<()> {
		req.funding.participant.verify_digest(req.hash(), &auth).or(Err(Error::NotAuthorized))?;
		self.withdraw_auth(&req, time)
	}

	pub fn withdraw_auth(self: &mut Self, req: &WithdrawalRequest, time: Timestamp) -> Result<()> {
		self.channels.get(&req.funding.channel).ok_or(Error::MalformedInput)?.guard_withdraw(time)?;
		self.approve_withdrawal(req)
	}

	fn approve_withdrawal(self: &mut Self, req: &WithdrawalRequest) -> Result<()> {
		let funds = self.clearFunds.remove(&req.funding).ok_or(Error::DuplicateWithdraw)?;
		// TODO: send funds to req.receiver.

		Ok(())
	}
}

#[test]
fn test_deposit() {
	let p = Perun::new();

	let acc = L2Account::default();
	let chan = Hasher::digest("a".as_bytes());

	let funding = Funding::new(chan, acc);
	let amount: Amount = Amount::from(213);
	p.deposit(funding, amount);

	assert!(p.deposits.contains_key(funding));
	assert_eq!(p.deposits.get(funding), Some(amount));
}