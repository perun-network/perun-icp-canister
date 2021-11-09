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

use types::*;
use error::*;
use ed25519_dalek::{SecretKey, PublicKey, ExpandedSecretKey};
use candid::Encode;

thread_local! {
	static STATE: RefCell<CanisterState> = Default::default();
}

#[derive(Default)]
/// The canister's state. Contains all currently registered channels, as well as
/// all deposits and withdrawable balances.
struct CanisterState {
	/// Tracks all deposits for unregistered channels. For registered channels,
	/// tracks withdrawable balances instead.
	deposits: HashMap<Funding, Amount>,
	/// Tracks all registered channels.
	_channels: HashMap<ChannelId, RegisteredState>,
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
fn conclude(_params: Params, _state: FullySignedState) -> () { }

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

	pub fn dispute(&self, params: Params, state: FullySignedState) -> Result<()> {
		for (i, pk) in params.participants.iter().enumerate() {
			state.state.validate_sig(&state.sigs[i], &pk)?;
		}
		Ok(())
	}
}


#[test]
fn test_deposit() {
	let mut canister = CanisterState::default();
	let funding = Funding::default();
	// Deposit 10.
	assert_eq!(canister.deposit(funding.clone(), 10.into()), Ok(()));
	// Now 10.
	assert_eq!(canister.query_deposit(funding.clone()), Some(10.into()));
	// Deposit 20.
	assert_eq!(canister.deposit(funding.clone(), 20.into()), Ok(()));
	// Now 30.
	assert_eq!(canister.query_deposit(funding), Some(30.into()));
}

static _SECRET_KEY_BYTES: [u8; 32] = [
	157, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068, 073, 197,
	105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
];

#[test]
fn test_dispute_sig() {
	let alice_sk = SecretKey::from_bytes(&_SECRET_KEY_BYTES).unwrap();
	let alice_esk = ExpandedSecretKey::from(&alice_sk);
	let alice_pk: PublicKey = (&alice_sk).into();
	let alice = L2Account(alice_pk);

	let canister = CanisterState::default();
	let hash = vec![123u8; 32];
	let state = State {
		channel: hash.clone(),
		version: 564,
		allocation: vec![10.into()],
		finalized: false,
	};
	let enc = Encode!(&state).unwrap();
	let alice_sig = L2Signature(alice_esk.sign(&enc, &alice_pk).to_bytes().into());

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
