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

use error::*;
use events::*;
use ic_cdk::api::time as blocktime;
use ic_cdk::export::Principal;
use ic_ledger_types::{
	AccountIdentifier, Memo, Tokens, TransferArgs, DEFAULT_FEE, DEFAULT_SUBACCOUNT,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::RwLock;
use types::*;

use candid::export_service;
use ic_cdk::export::candid::candid_method;
use ic_cdk_macros::{query, update};

#[query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
	export_service!();
	__export_service()
}

lazy_static! {
	static ref STATE: RwLock<CanisterState<icp::CanisterTXQuerier>> =
		RwLock::new(CanisterState::new(
			icp::CanisterTXQuerier::new(
				Principal::from_text("bkyz2-fmaaa-aaaaa-qaaaq-cai").expect("parsing principal")
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
#[candid::candid_method]
/// The user needs to call this with his transaction.
async fn transaction_notification(block_height: u64) -> Option<Amount> {
	STATE.write().unwrap().process_icp_tx(block_height).await
}

#[query]
#[candid::candid_method(query)]

/// Returns the funding specific for a channel's participant.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_funding_only(funding: Funding) -> Option<Funding> {
	Some(funding.clone())
}

#[query]
#[candid::candid_method(query)]
/// Returns only the memo specific for a channel.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_memo(mem: i32) -> Option<i32> {
	Some(mem)
}

#[query]
#[candid_method(query)]
/// Returns the funds deposited for a channel's specified participant, if any.
/// this function should be used to check whether all participants have
/// deposited their owed funds into a channel to ensure it is fully funded.
fn query_holdings(funding: Funding) -> Option<Amount> {
	STATE.read().unwrap().query_holdings(funding)
}

#[update]
#[candid_method]

async fn deposit(funding: Funding) -> Option<Error> {
	STATE
		.write()
		.unwrap()
		.deposit_icp(blocktime(), funding)
		.await
		.err()
}

#[update]
#[candid_method(update)]

/// Only used for tests.
fn deposit_mocked(funding: Funding, amount: Amount) -> Option<Error> {
	STATE.write().unwrap().deposit(funding, amount).err()
}

#[update]
#[candid_method(update)]
/// Starts a dispute settlement for a non-finalized channel. Other participants
/// will have to reply with a call to 'dispute' within the channel's challenge
/// duration to register a more recent channel state if exists. After the
/// challenge duration elapsed, the channel will be marked as settled.
async fn dispute(creq: ConcludeRequest) -> String {
	let params = Params {
		nonce: creq.nonce.clone(),
		participants: creq.participants.clone(),
		challenge_duration: creq.challenge_duration.clone(),
	};

	let bare_state = State {
		channel: creq.channel.clone(),
		version: creq.version.clone(),
		allocation: creq.allocation.clone(),
		finalized: creq.finalized.clone(),
	};

	let state = FullySignedState {
		state: bare_state.clone(),
		sigs: creq.sigs.clone(),
	};

	match STATE
		.write()
		.unwrap()
		.dispute(params, state, blocktime())
		.await
	{
		Ok(_) => "successful initialization of a dispute".to_string(),
		Err(_) => "error disputing".to_string(),
	}
}

#[update]
#[candid::candid_method]
fn verify_sig(creq: ConcludeRequest) -> String {
	let sigs = creq.sigs.clone();
	let addrs = creq.participants.clone();

	let bare_state = State {
		channel: creq.channel.clone(),
		version: creq.version.clone(),
		allocation: creq.allocation.clone(),
		finalized: creq.finalized.clone(),
	};

	for (i, pk) in addrs.iter().enumerate() {
		if let Err(_) = bare_state.validate_sig(&sigs[i], pk) {
			return "Signature verification failed".to_string();
		}
	}

	"Signatures verified successfully".to_string()
}

#[update]
#[candid::candid_method]
async fn conclude(conreq: ConcludeRequest) -> String {
	let params = Params {
		nonce: conreq.nonce.clone(),
		participants: conreq.participants.clone(),
		challenge_duration: conreq.challenge_duration.clone(),
	};

	let bare_state = State {
		channel: conreq.channel.clone(),
		version: conreq.version.clone(),
		allocation: conreq.allocation.clone(),
		finalized: conreq.finalized.clone(),
	};

	let state = FullySignedState {
		state: bare_state,
		sigs: conreq.sigs.clone(),
	};

	match STATE
		.write()
		.unwrap()
		.conclude(params, state, blocktime())
		.await
	{
		Ok(_) => "successful concluding the channel".to_string(),
		Err(_) => "error concluding the channel".to_string(),
	}
}

#[update]
#[candid::candid_method]
// Withdraws the specified participant's funds from a settled channel.
async fn withdraw(req: WithdrawalRequest) -> String {
	let result = withdraw_impl(req).await;

	match result {
		Ok(_block_height) => "successful withdrawal".to_string(),
		Err(_) => "error withdrawing".to_string(),
	}
}

#[update]
/// Withdraws the specified participant's funds from a settled channel (mocked)
async fn withdraw_mocked(request: WithdrawalRequest) -> (Option<Amount>, Option<Error>) {
	let result = STATE.write().unwrap().withdraw(request); // auth
	(result.as_ref().ok().cloned(), result.err())
}
async fn withdraw_impl(request: WithdrawalRequest) -> Result<icp::BlockHeight> {
	let receiver = request.receiver.clone();
	let funding = Funding {
		channel: request.channel.clone(),
		participant: request.participant.clone(),
	};

	let amount = STATE.write().unwrap().withdraw(request)?;

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
#[candid::candid_method(query)]
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
					timestamp: time,
				},
			)
			.await;
		Ok(())
	}

	/// Call this to process an ICP transaction and register the funds for
	/// further use.
	pub async fn process_icp_tx(&mut self, tx: icp::BlockHeight) -> Option<Amount> {
		match self.icp_receiver.verify(tx).await {
			Ok(v) => Some(v),
			Err(_e) => None, //Err(Error::ReceiverError(e)),
		}
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
		self.update_holdings(&params, &state.state);
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

	pub fn conclude_can(
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

	pub async fn conclude(
		&mut self,
		params: Params,
		fsstate: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.state(&fsstate.state.channel) {
			require!(!old_state.settled(now), AlreadyConcluded);
		}

		self.register_channel(
			&params,
			RegisteredState::conclude(fsstate.clone(), &params)?,
		)?;

		let state = fsstate.state.clone();
		let regstate = RegisteredState {
			state: state.clone(),
			timeout: now,
		};

		events::STATE
			.write()
			.unwrap()
			.register_event(
				now,
				state.channel.clone(),
				Event::Concluded {
					state: regstate,
					timestamp: now,
				},
			)
			.await;
		Ok(())
	}

	pub fn dispute_can(
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

	pub async fn dispute(
		&mut self,
		params: Params,
		fsstate: FullySignedState,
		now: Timestamp,
	) -> Result<()> {
		if let Some(old_state) = self.state(&fsstate.state.channel) {
			require!(!old_state.settled(now), AlreadyConcluded);
			require!(
				old_state.state.version < fsstate.state.version,
				OutdatedState
			);
		}

		self.register_channel(
			&params,
			RegisteredState::dispute(fsstate.clone(), &params, now)?,
		)?;

		let bare_state = State {
			channel: fsstate.state.channel.clone(),
			version: fsstate.state.version.clone(),
			allocation: fsstate.state.allocation.clone(),
			finalized: fsstate.state.finalized.clone(),
		};

		let regstate = RegisteredState {
			state: bare_state.clone(),
			timeout: now + to_nanoseconds(params.challenge_duration), //params.challenge_duration * 1_000_000_000,
		};

		match events::STATE.write() {
			Ok(mut state) => {
				state
					.register_event(
						now,
						bare_state.channel.clone(),
						Event::Disputed {
							state: regstate,
							timestamp: now,
						},
					)
					.await
			}
			Err(_) => return Err(Error::InvalidInput),
		}

		Ok(())
	}

	pub fn withdraw(&mut self, req: WithdrawalRequest) -> Result<Amount> {
		let auth = req.signature.clone();
		let now = req.time.clone();
		req.validate_sig(&auth)?;
		let funding = Funding::new(req.channel.clone(), req.participant.clone());
		match self.state(&req.channel) {
			None => Err(Error::NotFinalized),
			Some(state) => {
				require!(state.settled(now), NotFinalized);
				Ok(self.holdings.remove(&funding).unwrap_or_default())
			}
		}
	}

	pub fn withdraw_can(
		&mut self,
		req: WithdrawalTestRq,
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

pub fn hash_to_channel_id(hash: &Hash) -> ChannelId {
	let mut arr = [0u8; 32];
	arr.copy_from_slice(&hash.0[..32]);
	ChannelId(arr)
}
