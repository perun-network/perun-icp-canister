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

use crate::*;

//use std::hash::Hash;
use core::cmp::*;
use ecdsa::hazmat::DigestPrimitive;
pub use ic_cdk::export::candid::{CandidType, Deserialize, Int, Nat};

use k256::Secp256k1 as Curve;
use ecdsa::VerifyingKey;
pub use ecdsa::signature::DigestVerifier;
use digest::Output;

use ecdsa::Signature;

/// A layer-2 signature.
pub type L2Signature = Signature<Curve>;
/// A hasher.
pub type Hasher = <Curve as DigestPrimitive>::Digest;
/// A hash as used by the signature scheme.
pub type Hash = Output<Hasher>;
#[derive(PartialEq, Eq, CandidType)]
/// A layer-2 account identifier.
pub struct L2Account(VerifyingKey<Curve>);
/// A payable layer-1 account identifier.
pub use ic_cdk::export::candid::Principal as L1Account;
/// An amount of a currency.
pub type Amount = Nat;
/// Duration in seconds.
pub type Duration = u64;
/// UNIX timestamp.
pub type Timestamp = u64;
/// Unique Perun channel identifier.
pub type ChannelId = Hash;
/// A channel's unique nonce.
pub type Nonce = Hash;
/// Channel state version identifier.
pub type Version = u64;

#[derive(CandidType)]
pub struct Params {
	pub none: Nonce,
	pub participants: Vec<L2Account>,
	pub challenge_duration: Duration,
}

pub struct State {
	pub channel: ChannelId,
	pub version: Version,
	pub allocation: Vec<Amount>,
	pub finalized: bool,
}

pub struct FullySignedState {
	pub state: State,
	pub sigs: Vec<L2Signature>,
}

pub struct RegisteredState {
	pub state: State,
	pub timeout: Timestamp,
}

pub struct WithdrawalRequest {
	pub funding: Funding,
	pub receiver: L1Account,
}

#[derive(PartialEq, Eq, Hash)]
pub struct Funding {
	pub channel: ChannelId,
	pub participant: L2Account,
}

impl std::hash::Hash for L2Account {
	fn hash<H: std::hash::Hasher>(self: &Self, state: &mut H) {
		self.0.to_encoded_point(true).as_bytes().hash(state);
	}
}

impl DigestVerifier<Hasher, L2Signature> for L2Account {
	fn verify_digest(self: &Self, digest: Hasher, signature: &L2Signature) -> ecdsa::Result<()> {
		self.0.verify_digest(digest, signature)
	}
}

impl CandidType for L2Account {
	fn idl_serialize<S>(&self, serializer: S) -> core::result::Result<(), S::Error>
	where S: ic_cdk::export::candid::types::Serializer {
	}
}

impl Params {
	pub fn idx(self: &Self, find: L2Account) -> bool {
		for (i, acc) in self.participants.into_iter().enumerate() {
			if acc == find {
				return true;
			}
		}
		return false;
	}

	pub fn id(self: &Self) -> ChannelId {
		return ChannelId::default();
	}

	pub fn matches(self: &Self, state: &State) -> bool {
		return self.id() == state.channel &&
			self.participants.len() == state.allocation.len();
	}
}

impl State {
	pub fn hash(self: &Self) -> Hasher {
		let x = Hasher::default();
		return x;
	}
}

impl FullySignedState {
	pub fn verify(self: &Self, params: &Params) -> Result<()> {
		if params.participants.len() != self.sigs.len() {
			return Err(Error::MalformedInput);
		}

		let hash = self.state.hash();

		for (i, part) in params.participants.into_iter().enumerate() {
			if let Err(_) = part.verify_digest(hash, &self.sigs[i]) {
				return Err(Error::InvalidSignatures);
			}
		}
		Ok(())
	}
}

impl RegisteredState {
	pub fn settled(state: &State) -> Self {
		return Self{
			state: *state,
			timeout: 0,
		}
	}

	pub fn guard_withdraw(self: &Self, time: Timestamp) -> Result<()> {
		if !self.state.finalized && time < self.timeout {
			return Err(Error::NotAuthorized);
		}
		return Ok(());
	}
}

impl WithdrawalRequest {
	pub fn hash(self: &Self) -> Hasher {
		let h = Hasher::default();
		// TODO: encode into hasher.
		return h;
	}
}

impl Funding {
	pub fn new(channel: ChannelId, participant: L2Account) -> Self {
		Self{
			channel: channel,
			participant: participant,
		}
	}
}