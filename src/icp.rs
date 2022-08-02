//  Copyright 2022 PolyCrypt GmbH
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

use crate::types::Amount;
use async_trait::async_trait;
use ic_cdk::export::candid::{CandidType, Deserialize};
use ic_cdk::export::Principal;
use ic_ledger_types::{
	query_archived_blocks, query_blocks, AccountIdentifier, Block, GetBlocksArgs, Operation,
	Transaction, DEFAULT_SUBACCOUNT,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAINNET_ICP_LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

pub type Memo = u64;
pub type BlockHeight = u64;

/// ICP token handling errors.
#[derive(PartialEq, Eq, CandidType, Deserialize, Debug)]
pub enum ICPReceiverError {
	TransactionType,
	Recipient,
	DuplicateTransaction,
	FailedToQuery,
}

impl std::fmt::Display for ICPReceiverError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		std::fmt::Debug::fmt(self, f)
	}
}

/// ICP transaction receiver for receiving and tracking payments for separate purposes.
pub struct Receiver<Q: TXQuerier> {
	tx_querier: Q,
	my_account: AccountIdentifier,
	known_txs: BTreeSet<BlockHeight>, // set of block heights
	unspent: BTreeMap<Memo, Amount>,  // received tokens per memo
}

/// ICP transaction querier.
#[async_trait]
pub trait TXQuerier {
	/// Allows the
	async fn query_tx(&self, block_height: BlockHeight) -> Result<TransactionNotification, ICPReceiverError>;
}

/// Mocked ICP transaction querier for simulation and testing purposes.
#[derive(Default)]
pub struct MockTXQuerier {
	txs: BTreeMap<BlockHeight, TransactionNotification>,
}

#[async_trait]
impl TXQuerier for MockTXQuerier {
	async fn query_tx(&self, block_height: BlockHeight) -> Result<TransactionNotification, ICPReceiverError> {
		self.txs.get(&block_height).cloned().ok_or(ICPReceiverError::FailedToQuery)
	}
}

impl MockTXQuerier {
	/// Inserts a transaction so that it can be read via query_tx().
	pub fn register_tx(&mut self, block_height: BlockHeight, tx: TransactionNotification) {
		self.txs.insert(block_height, tx);
	}
}

/// Real ICP transaction querier using inter-canister calls to the ICP ledger.
pub struct CanisterTXQuerier {
	icp_ledger: Principal,
}

#[async_trait]
impl TXQuerier for CanisterTXQuerier {
	async fn query_tx(&self, block_height: BlockHeight) -> Result<TransactionNotification, ICPReceiverError> {
		if let Some(block) = self.get_block_from_ledger(block_height).await {
			if let Some(tx) = TransactionNotification::from_tx(block.transaction) {
				return Ok(tx);
			} else {
				return Err(ICPReceiverError::TransactionType);
			}
		}
		Err(ICPReceiverError::FailedToQuery)
	}
}

impl CanisterTXQuerier {
	pub fn new(ledger: Principal) -> Self {
		Self { icp_ledger: ledger }
	}

	/// Constructs a new canister TX querier targeting the mainnet ICP ledger canister.
	pub fn for_mainnet() -> Self {
		Self {
			icp_ledger: Principal::from_text(MAINNET_ICP_LEDGER).unwrap(),
		}
	}

	/// Queries a block from the ICP ledger's internal blockchain.
	async fn get_block_from_ledger(&self, block_height: BlockHeight) -> Option<Block> {
		let args = GetBlocksArgs {
			start: block_height,
			length: 1,
		};
		if let Ok(result) = query_blocks(self.icp_ledger, args.clone()).await {
			if result.blocks.len() != 0 {
				return result.blocks.first().cloned();
			}
			if let Some(b) = result
				.archived_blocks
				.into_iter()
				.find(|b| (b.start <= block_height && (block_height - b.start) < b.length))
			{
				if let Ok(Ok(range)) = query_archived_blocks(&b.callback, args).await {
					return range.blocks.get((block_height - b.start) as usize).cloned();
				}
			}
		}
		None
	}
}

impl<Q> Receiver<Q>
where
	Q: TXQuerier,
{
	/// Creates a new transaction receiver for the specified canister principal.
	pub fn new(q: Q, my_principal: Principal) -> Self {
		Self {
			tx_querier: q,
			my_account: AccountIdentifier::new(&my_principal, &DEFAULT_SUBACCOUNT),
			known_txs: Default::default(),
			unspent: Default::default(),
		}
	}

	/// Verifies a transaction, and if it's valid and new, tracks its funds and
	/// returns its amount.
	pub async fn verify(
		&mut self,
		block_height: BlockHeight,
	) -> std::result::Result<Amount, ICPReceiverError> {
		if self.known_txs.contains(&block_height) {
			return Err(ICPReceiverError::DuplicateTransaction);
		}

		match self.tx_querier.query_tx(block_height).await {
			Ok(tx) => {
				if !self.known_txs.insert(block_height) {
					return Err(ICPReceiverError::DuplicateTransaction);
				}
				if tx.to != self.my_account {
					return Err(ICPReceiverError::Recipient);
				}
				*self.unspent.entry(tx.memo).or_insert(0.into()) += tx.get_amount();

				Ok(tx.get_amount())
			},
			Err(e) => Err(e)
		}
	}

	/// Withdraws all funds from the requested memo.
	pub fn drain(&mut self, memo: Memo) -> Amount {
		return self.unspent.remove(&memo).unwrap_or(0.into()).into();
	}

	/// Withdraws all funds from the requested memo if it is above a threshold.
	pub fn drain_if_at_least(&mut self, memo: Memo, amount: Amount) -> Option<Amount> {
		if let Some(sum) = self.unspent.get(&memo) {
			if sum >= &amount {
				return self.unspent.remove(&memo).unwrap().into();
			}
		}
		None
	}
}

/// Contents of a received transaction.
#[derive(Clone, Hash, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct TransactionNotification {
	pub to: AccountIdentifier,
	pub amount: u64,
	pub memo: Memo,
}

impl TransactionNotification {
	/// Creates a transaction notification from an ICP ledger transaction. If the transaction is neither a transfer nor a mint, returns nothing.
	pub fn from_tx(tx: Transaction) -> Option<Self> {
		if tx.operation.is_none() {
			return None;
		}

		match tx.operation.unwrap() {
			Operation::Transfer { to, amount, .. } => {
				return Some(Self {
					to: to,
					amount: amount.e8s(),
					memo: tx.memo.0,
				});
			}
			Operation::Mint { to, amount, .. } => {
				return Some(Self {
					to: to,
					amount: amount.e8s(),
					memo: tx.memo.0,
				});
			}
			_ => (),
		}
		None
	}

	/// Returns the transaction's amount.
	pub fn get_amount(&self) -> Amount {
		self.amount.into()
	}
}
