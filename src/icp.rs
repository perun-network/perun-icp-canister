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

use crate::{
	types::{Hash, Amount},
};
use ic_cdk::export::candid::{Encode, CandidType, Deserialize};
use ic_cdk::export::Principal;
use std::collections::{BTreeSet, BTreeMap};
use ic_ledger_types::{
	GetBlocksArgs,
	Block,
	Transaction,
	Operation,
	AccountIdentifier,
	DEFAULT_SUBACCOUNT,
	query_blocks,
	query_archived_blocks
};
use async_trait::async_trait;

pub const MAINNET_ICP_LEDGER: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

pub type Memo = u64;
pub type BlockHeight = u64;

pub struct Receiver<Q: TXQuerier> {
	tx_querier: Q,
	my_account: AccountIdentifier,
	known_txs: BTreeSet<BlockHeight>, // set of block heights
	unspent: BTreeMap<Memo, Amount>, // received tokens per memo
}

#[async_trait]
pub trait TXQuerier {
	async fn query_tx(&self, block_height: u64) -> Option<TransactionNotification>;
}

#[derive(Default)]
pub struct MockTXQuerier {
	txs: BTreeMap<u64, TransactionNotification>,
}

#[async_trait]
impl TXQuerier for MockTXQuerier {
	async fn query_tx(&self, block_height: u64) -> Option<TransactionNotification> {
		self.txs.get(&block_height).cloned()
	}
}

pub struct CanisterTXQuerier {
	icp_ledger: Principal,
}

#[async_trait]
impl TXQuerier for CanisterTXQuerier {
	async fn query_tx(&self, block_height: BlockHeight) -> Option<TransactionNotification> {
		if let Some(block) = self.get_block_from_ledger(block_height).await {
			return TransactionNotification::from_tx(block.transaction);
		}
		None
	}
}

impl CanisterTXQuerier {
	pub fn new(ledger: Principal) -> Self {
		Self { icp_ledger: ledger }
	}

	async fn get_block_from_ledger(&self, block_height: BlockHeight) -> Option<Block> {
		let args = GetBlocksArgs{ start: block_height, length: 1 };
		if let Ok(result) = query_blocks(self.icp_ledger, args.clone()).await {
			if result.blocks.len() != 0 {
				return result.blocks.first().cloned()
			}
			if let Some(b) = result.archived_blocks
				.into_iter()
				.find(|b| (b.start <= block_height && (block_height - b.start) < b.length)) {
				if let Ok(Ok(range)) = query_archived_blocks(&b.callback, args).await {
					return range.blocks.get((block_height - b.start) as usize).cloned()
				}
			}
		}
		None
	}
}

impl<Q> Receiver<Q> where Q:TXQuerier {
	pub fn new(q: Q, my_principal: Principal) -> Self {
		Self {
			tx_querier: q,
			my_account: AccountIdentifier::new(&my_principal, &DEFAULT_SUBACCOUNT),
			known_txs: Default::default(),
			unspent: Default::default(),
		}
	}

	/// Verifies a transaction, and if it is valid and new, tracks its funds.
	pub async fn verify(&mut self, block_height: BlockHeight) -> bool {
		if self.known_txs.contains(&block_height) {
			return false;
		}

		if let Some(tx) = self.tx_querier.query_tx(block_height).await {
			if !self.known_txs.insert(block_height) {
				return false;
			}
			if tx.to != self.my_account {
				return false;
			}
			*self.unspent.entry(tx.memo).or_insert(0.into()) += tx.get_amount();
			return true;
		}
		false
	}

	/// Withdraws all funds from the requested memo.
	pub fn drain(&mut self, memo: Memo) -> Amount {
		return self.unspent.remove(&memo).unwrap_or(0.into()).into()
	}
}


/// Contents of a received transaction.
#[derive(Clone, Hash, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct TransactionNotification {
	pub to: AccountIdentifier,
	pub amount: u64,
	pub memo: u64,
}

impl TransactionNotification {
	pub fn from_tx(tx: Transaction) -> Option<Self> {
		if tx.operation.is_none() {
			return None;
		}

		match tx.operation.unwrap() {
			Operation::Transfer{to, amount, ..} => {
				return Some(Self{
					to: to,
					amount: amount.e8s(),
					memo: tx.memo.0,
				});
			},
			_ => (),
		}
		None
	}

	pub fn hash(&self) -> Hash {
		Hash::digest(&Encode!(self).unwrap())
	}

	pub fn get_amount(&self) -> Amount {
		self.amount.into()
	}
}
