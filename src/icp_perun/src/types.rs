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

use core::cmp::*;
use candid::Encode;
use crate::error::{
	Error as CError,
	Result as CanisterResult,
};
pub use ic_cdk::export::candid::{
	CandidType, Deserialize,
	Int, Nat,
	types::{
		Serializer,
		Type,
	},
};
use ed25519_dalek::{PublicKey, Signature};
use serde::de::{Deserializer, Visitor, Error};


// Type definitions start here.


/// A hash as used by the signature scheme.
pub type Hash = Vec<u8>;

#[derive(PartialEq, Default, Clone, Eq)]
/// A layer-2 account identifier.
pub struct L2Account(pub PublicKey);

#[derive(PartialEq, Clone, Eq)]
// A layer-2 signature for signing Perun protocol messages.
pub struct L2Signature(pub Signature);

/// A payable layer-1 account identifier. Could be both a user or a canister.
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

#[derive(Deserialize, CandidType)]
/// The immutable parameters and state of a Perun channel.
pub struct Params {
	/// The channel's unique nonce, to protect against replay attacks.
	pub nonce: Nonce,
	/// The channel's participants' layer-2 identities.
	pub participants: Vec<L2Account>,
	/// When a dispute occurs, how long to wait for responses.
	pub challenge_duration: Duration,
}

#[derive(Deserialize, CandidType)]
/// The mutable parameters and state of a Perun channel. Contains 
pub struct State {
	/// The cannel's unique identifier.
	pub channel: ChannelId,
	/// The channel's current state revision number.
	pub version: Version,
	/// The channel's asset allocation. Contains each participant's current
	/// balance in the order of the channel parameters' participant list.
	pub allocation: Vec<Amount>,
	/// Whether the channel is finalized, i.e., no more updates can be made and
	/// funds can be withdrawn immediately. A non-finalized channel has to be
	/// finalized via the canister after the channel's challenge duration
	/// elapses.
	pub finalized: bool,
}

#[derive(Deserialize, CandidType)]
/// A channel state, signed by all participants.
pub struct FullySignedState {
	/// The channel's state.
	pub state: State,
	/// The channel's participants' signatures on the channel state, in the
	/// order of the parameters' participant list.
	pub sigs: Vec<L2Signature>,
}

#[derive(Deserialize, CandidType)]
/// A registered channel's state, as seen by the canister. Represents a channel
/// after a call to "conclude" or "dispute" on the canister. The timeout, in
/// combination with the state's "finalized" flag determine whether a channel is
/// concluded and its funds ready for withdrawing.
pub struct RegisteredState {
	/// The channel's state, containing challenge duration, outcomes, and
	/// whether the channel is already finalized.
	pub state: State,
	/// The challenge timeout after which the currently registered state becomes
	/// available for withdrawing. Ignored for finalized channels.
	pub timeout: Timestamp,
}

#[derive(Deserialize, CandidType)]
/// Contains the payload of a request to withdraw a participant's funds from a
/// registered channel. Does not contain the authorization signature.
pub struct WithdrawalRequest {
	/// The funds to be withdrawn.
	pub funding: Funding,
	/// The layer-1 identity to send the funds to.
	pub receiver: L1Account,
}

#[derive(PartialEq, Clone, Default, Deserialize, Eq, Hash, CandidType)]
/// Identifies the funds belonging to a certain layer 2 identity within a
/// certain channel.
pub struct Funding {
	/// The channel's unique identifier.
	pub channel: ChannelId,
	/// The funds' owner's layer-2 identity within the channel.
	pub participant: L2Account,
}

// L2Account

impl<'de> Deserialize<'de> for L2Account {
	fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
	where D: Deserializer<'de> {
		let bytes: &[u8] = &deserializer.deserialize_bytes(BlobDecoderVisitor::default())?;
		let pk = PublicKey::from_bytes(bytes).ok().ok_or(
			D::Error::invalid_length(bytes.len(), &"public key"))?;
		Ok(L2Account(pk))
	}
}

impl CandidType for L2Account {
	fn _ty() -> Type { Type::Vec(Box::new(Type::Nat8)) }

	fn idl_serialize<S>(&self, serializer: S) -> core::result::Result<(), S::Error>
	where S: Serializer {
		serializer.serialize_blob(&self.0.to_bytes())
	}
}


impl std::hash::Hash for L2Account {
	fn hash<H: std::hash::Hasher>(self: &Self, state: &mut H) {
		self.0.to_bytes().hash(state);
	}
}

// L2Signature

impl<'de> Deserialize<'de> for L2Signature {
	fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
	where D: Deserializer<'de> {
		let bytes: &[u8] = &deserializer.deserialize_bytes(BlobDecoderVisitor::default())?;
		let bytes64 = as_bytes64(bytes).ok_or(
			D::Error::invalid_length(bytes.len(), &"signature"))?;
		let sig = Signature::new(bytes64);
		Ok(L2Signature(sig))
	}
}


impl CandidType for L2Signature {
	fn _ty() -> Type { Type::Vec(Box::new(Type::Nat8)) }

	fn idl_serialize<S>(&self, serializer: S) -> core::result::Result<(), S::Error>
	where S: Serializer {
		serializer.serialize_blob(&self.0.to_bytes())
	}
}


// State

impl State {
	pub fn validate_sig(&self, sig: &L2Signature, pk: &L2Account) -> CanisterResult<()> {
		let enc = Encode!(self).expect("encoding state");
		pk.0.verify_strict(&enc, &sig.0).ok().ok_or(CError::Authentication)
	}
}

// Params

impl Params {
	pub fn id(self: &Self) -> ChannelId {
		return ChannelId::default();
	}

	pub fn matches(self: &Self, state: &State) -> bool {
		return self.id() == state.channel && self.participants.len() == state.allocation.len();
	}
}

// FullySignedState

impl FullySignedState {
	pub fn validate(&self, params: &Params) -> CanisterResult<()> {
		if self.state.channel != params.id() {
			Err(CError::InvalidInput)?;
		}
		for (i, pk) in params.participants.iter().enumerate() {
			self.state.validate_sig(&self.sigs[i], &pk)?;
		}
		Ok(())
	}

	pub fn validate_final(&self, params: &Params) -> CanisterResult<()> {
		if !self.state.finalized {
			Err(CError::NotFinalized)?;
		}
		self.validate(params)
	}
}

// RegisteredState

impl RegisteredState {
	pub fn conclude(state: FullySignedState, params: &Params) -> CanisterResult<Self> {
		state.validate_final(params)?;
		Ok(Self {
			state: state.state,
			timeout: Default::default(),
		})
	}

	pub fn dispute(state: FullySignedState, params: &Params, now: Timestamp) -> CanisterResult<Self> {
		state.validate(params)?;
		Ok(Self{
			state: state.state,
			timeout: now + params.challenge_duration,
		})
	}

	pub fn settled(&self, now: Timestamp) -> bool {
		self.state.finalized || now >= self.timeout
	}
}


// Funding

impl Funding {
	pub fn new(channel: ChannelId, participant: L2Account) -> Self {
		Self {
			channel: channel,
			participant: participant,
		}
	}
}


// Miscellaneous helpers

/// Needed to decode a blob into a public key's 64 byte
fn as_bytes64(v: &[u8]) -> Option<[u8;64]> {
	if v.len() != 64 {
		return None;
	}

	let mut ret: [u8;64] = [0; 64];
	let mut i = 0;
	while i < 64 {
		ret[i] = v[i];
		i += 1;
	}

	Some(ret)
}

#[derive(Default)]
/// Used as a helper for decoding a "blob" from candid data.
struct BlobDecoderVisitor {}

impl<'de> Visitor<'de> for BlobDecoderVisitor {
	type Value = Vec<u8>;
	fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(formatter, "expected blob")
	}
}
