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
pub mod error;
pub mod types;
use error::*;
use num_bigint::BigUint;
use std::cell::RefCell;
use types::*;

use ic_cdk::api::{caller, time};

thread_local! {
	static STATE: CanisterState = Default::default();
}

#[derive(Default)]
struct CanisterState {
	deposits: RefCell<HashMap<Funding, Amount>>,
	channels: RefCell<HashMap<ChannelId, RegisteredState>>,
}

#[ic_cdk_macros::update]
fn deposit(funding: Funding, amount: Amount) {
	STATE.with(|s| {
		*s.deposits
			.borrow_mut()
			.entry(funding)
			.or_insert(Default::default()) += amount
	});
	()
}

#[ic_cdk_macros::update]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as finalized.
fn dispute(params: Params, state: FullySignedState) -> () {}

#[ic_cdk_macros::update]
/// Settles a finalized channel. It can then be used for withdrawing.
fn conclude(params: Params, state: FullySignedState) -> () {}

#[ic_cdk_macros::update]
/// Withdraws funds from a settled channel.
fn withdraw(withdrawal: WithdrawalRequest, withdrawal_sig: L2Signature) -> () {}

#[ic_cdk_macros::query]
fn query_deposit(funding: Funding) -> Option<Amount> {
	STATE.with(|s| {
		let deposits = s.deposits.borrow();
		match deposits.get(&funding) {
			None => None,
			Some(a) => Some(a.clone()),
		}
	})
}

#[test]
fn test_deposit() {
	STATE.with(|_| {}); // init
	let funding = Funding::default();
	// Deposit 10.
	let ten: Amount = 10.into();
	deposit(funding.clone(), ten.clone());
	// Now 10.
	assert_eq!(query_deposit(funding.clone()), Some(ten));
	// Deposit 20.
	let twenty: Amount = 20.into();
	deposit(funding.clone(), twenty.clone());
	// Now 30.
	assert_eq!(query_deposit(funding), Some(30.into()));
}
