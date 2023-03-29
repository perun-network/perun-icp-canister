//  Copyright 2021, 2022 PolyCrypt GmbH
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
pub mod events;
pub mod icp;
pub mod types;

// We don't need testing code in wasm output, only for tests and examples
#[cfg(not(target_family = "wasm"))]
pub mod test;
// The actual canister tests
#[cfg(test)]
mod tests;

use ic_cdk::api::time as blocktime;
use ic_cdk::export::Principal;
use ic_ledger_types::{
	AccountIdentifier, Memo, Tokens, TransferArgs, DEFAULT_FEE, DEFAULT_SUBACCOUNT,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;

use error::*;
use events::*;
use types::*;

lazy_static! {
	static ref STATE: RwLock<CanisterState<icp::CanisterTXQuerier>> =
		RwLock::new(CanisterState::new(
			icp::CanisterTXQuerier::new(
				Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").expect("parsing principal")
			),
			ic_cdk::id(),
		));
}

/// The canister's state. Contains all currently registered channels, as well as
/// all deposits and withdrawable balances.
pub struct CanisterState<Q: icp::TXQuerier> {
	icp_receiver: icp::Receiver<Q>,
	/// Tracks all deposits for unregistered channels. For registered channels,
	/// tracks withdrawable balances instead.
	holdings: HashMap<Funding, Amount>,
	/// Tracks all registered channels.
	channels: HashMap<ChannelId, RegisteredState>,
}

#[ic_cdk_macros::update]
/// The user needs to call this with his transaction.
async fn transaction_notification(block_height: u64) -> Result<Amount> {
	STATE.write().unwrap().process_icp_tx(block_height).await
}

#[ic_cdk_macros::update]
async fn deposit_memo(fundmem: FundMem) -> Option<Error> {
	STATE
		.write()
		.unwrap()
		.deposit_icp_memo(blocktime(), fundmem)
		.await
		.err()
}

#[ic_cdk_macros::update]
async fn deposit(funding: Funding) -> Option<Error> {
	STATE
		.write()
		.unwrap()
		.deposit_icp(blocktime(), funding)
		.await
		.err()
}

#[ic_cdk_macros::update]
/// Only used for tests.
fn deposit_mocked(funding: Funding, amount: Amount) -> Option<Error> {
	STATE.write().unwrap().deposit(funding, amount).err()
}

#[ic_cdk_macros::update]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as settled.
fn dispute(params: Params, state: FullySignedState) -> Option<Error> {
	STATE
		.write()
		.unwrap()
		.dispute(params, state, blocktime())
		.err()
}

#[ic_cdk_macros::update]
/// Settles a finalized channel and makes its final funds distribution
/// withdrawable.
fn conclude(params: Params, state: FullySignedState) -> Option<Error> {
	STATE
		.write()
		.unwrap()
		.conclude(params, state, blocktime())
		.err()
}

#[ic_cdk_macros::update]
/// Withdraws the specified participant's funds from a settled channel.
async fn withdraw(
	request: WithdrawalRequest,
	auth: L2Signature,
) -> (Option<icp::BlockHeight>, Option<Error>) {
	let result = withdraw_impl(request, auth).await;
	(result.as_ref().ok().cloned(), result.err())
}

#[ic_cdk_macros::update]
/// Withdraws the specified participant's funds from a settled channel.
async fn withdraw_mocked(
	request: WithdrawalRequest,
	auth: L2Signature,
) -> (Option<Amount>, Option<Error>) {
	let result = STATE.write().unwrap().withdraw(request, auth, blocktime());
	(result.as_ref().ok().cloned(), result.err())
}


async fn withdraw_impl(request: WithdrawalRequest, auth: L2Signature) -> Result<icp::BlockHeight> {
	let receiver = request.receiver.clone();
	let funding = request.funding.clone();
	let amount = STATE
		.write()
		.unwrap()
		.withdraw(request, auth, blocktime())?;
	let mut amount_str = amount.to_string();
	amount_str.retain(|c| c != '_');
	let amount_u64 = amount_str.parse::<u64>().unwrap();

	let prince = Principal::from_text(icp::MAINNET_ICP_LEDGER).unwrap();

	println!("Principal: {:?}", prince);

	match ic_ledger_types::transfer(
		prince,
		TransferArgs {
			memo: Memo(0),
			amount: Tokens::from_e8s(amount_u64),
			fee: DEFAULT_FEE,
			from_subaccount: None,
			to: AccountIdentifier::new(&receiver, &DEFAULT_SUBACCOUNT),
			created_at_time: None,
		},
	)
	.await
	{
		Ok(transfer_result) => match transfer_result {
			Ok(block) => Ok(block.into()),
			Err(_) => {
				STATE.write().unwrap().deposit(funding, amount)?;
				Err(Error::LedgerError)
			}
		},
		_ => {
			STATE.write().unwrap().deposit(funding, amount)?;
			Err(Error::LedgerError)
		}
	}
}



#[ic_cdk_macros::query]
/// Returns the funds deposited for a channel's specified participant, if any.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_holdings(funding: Funding) -> Option<Amount> {
	STATE.read().unwrap().query_holdings(funding)
}

#[ic_cdk_macros::query]
/// Returns the memo specific for a channel's participant.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_fid(funding: Funding) -> Option<Memo> {
    STATE.read().unwrap().show_fid(funding)
}

#[ic_cdk_macros::query]
/// Returns only the memo specific for a channel.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_memo(mem: Memo) -> Option<Memo> {
    Some(mem)
}

#[ic_cdk_macros::query]
/// Returns the funding and memo specific for a channel's participant.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_funding_memo(fundmem: FundMem) -> Option<FundMem> {
    Some(fundmem)
}

#[ic_cdk_macros::query]
/// Returns the funding specific for a channel's participant.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_funding_only(funding: Funding) -> Option<Funding> {
    Some(funding.clone())
}

#[ic_cdk_macros::query]
/// Returns the latest registered state for a given channel and its dispute
/// timeout. This function should be used to check for registered disputes.
fn query_state(id: ChannelId) -> Option<RegisteredState> {
	STATE.read().unwrap().state(&id)
}

impl<Q> CanisterState<Q>
where
	Q: icp::TXQuerier,
{
	pub fn new(q: Q, my_principal: Principal) -> Self {
		Self {
			icp_receiver: icp::Receiver::new(q, my_principal),
			holdings: Default::default(),
			channels: Default::default(),
		}
	}
	pub fn deposit(&mut self, funding: Funding, amount: Amount) -> Result<()> {
		*self.holdings.entry(funding).or_insert(Default::default()) += amount;
		Ok(())
	}

	/// Call this to access funds deposited and previously registered - memo is in the input already
	pub async fn deposit_icp_memo(&mut self, time: Timestamp, fundmem: FundMem) -> Result<()> {

		let funding = Funding {
			channel: fundmem.channel.clone(),
			participant: fundmem.participant.clone(),
		};
		let amount = self.icp_receiver.drain(fundmem.memo.0);

		self.deposit(funding.clone(), amount)?;
		events::STATE
			.write()
			.unwrap()
			.register_event(
				time,
				funding.channel.clone(),
				Event::Funded {
					who: funding.participant.clone(),
					total: self.holdings.get(&funding).cloned().unwrap(), // here include unwrap_or for error handling/propagation
				},
			)
			.await;
		Ok(())
	}

	/// Call this to access funds deposited and previously registered.
	pub async fn deposit_icp(&mut self, time: Timestamp, funding: Funding) -> Result<()> {
		let memo = funding.memo();
		let amount = self.icp_receiver.drain(memo);
		self.deposit(funding.clone(), amount)?;
		events::STATE
			.write()
			.unwrap()
			.register_event(
				time,
				funding.channel.clone(),
				Event::Funded {
					who: funding.participant.clone(),
					total: self.holdings.get(&funding).cloned().unwrap(),
				},
			)
			.await;
		Ok(())
	}

	/// Call this to process an ICP transaction and register the funds for
	/// further use.
	pub async fn process_icp_tx(&mut self, tx: icp::BlockHeight) -> Result<Amount> {
		match self.icp_receiver.verify(tx).await {
			Ok(v) => Ok(v),
			Err(e) => Err(Error::ReceiverError(e)),
		}
	}

	pub fn query_holdings(&self, funding: Funding) -> Option<Amount> {
		self.holdings.get(&funding).cloned()
	}

	pub fn show_fid(&self, funding: Funding) -> Option<Memo> {
		let mem: u64 = funding.memo();
		Some(ic_ledger_types::Memo(mem))
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
