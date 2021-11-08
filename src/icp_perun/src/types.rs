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

use core::cmp::*;
//use ecdsa::hazmat::DigestPrimitive;
use candid::{Decode, Encode};
pub use ic_cdk::export::candid::{CandidType, Deserialize, Int, Nat};

use digest::Output;
//pub use ecdsa::signature::DigestVerifier;
//use ecdsa::signature::Signature;
//use ecdsa::{EncodedPoint, VerifyingKey};
//use k256::Secp256k1 as Curve;

/// A hasher.
//pub type Hasher = <Curve as DigestPrimitive>::Digest;
/// A hash as used by the signature scheme.
pub type Hash = Vec<u8>;
#[derive(PartialEq, Default, Clone, Eq, CandidType, Deserialize)]
/// A layer-2 account identifier.
pub struct L2Account(pub Vec<u8>);
#[derive(PartialEq, Default, Clone, Eq, CandidType, Deserialize)]
pub struct L2Signature(pub Vec<u8>);
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

#[derive(Deserialize, CandidType)]
pub struct Params {
	pub nonce: Nonce,
	pub participants: Vec<L2Account>,
	pub challenge_duration: Duration,
}

#[derive(Deserialize, CandidType)]
pub struct State {
	pub channel: ChannelId,
	pub version: Version,
	pub allocation: Vec<Amount>,
	pub finalized: bool,
}

#[derive(Deserialize, CandidType)]
pub struct FullySignedState {
	pub state: State,
	pub sigs: Vec<L2Signature>,
}

#[derive(Deserialize, CandidType)]
pub struct RegisteredState {
	pub state: State,
	pub timeout: Timestamp,
}

#[derive(Deserialize, CandidType)]
pub struct WithdrawalRequest {
	pub funding: Funding,
	pub receiver: L1Account,
}

#[derive(PartialEq, Clone, Default, Deserialize, Eq, Hash, CandidType)]
pub struct Funding {
	pub channel: ChannelId,
	pub participant: L2Account,
}

impl std::hash::Hash for L2Account {
	fn hash<H: std::hash::Hasher>(self: &Self, state: &mut H) {
		self.0.hash(state);
	}
}

impl State {
	pub fn validate_sig(&self, sig: &L2Signature, pk: &L2Account) {
		let enc = Encode!(self).expect("encoding state");
		let pk = PublicKey::from_bytes(&pk.0).expect("invalid pk");
		let sig = ed25519::signature::Signature::from_bytes(&sig.0).expect("invalid sig");
		pk.verify(&enc, &sig).expect("wrong sig")
	}
}

/*
impl L2Account {
	pub fn verify_digest(
		self: &Self,
		digest: Hasher,
		signature: &L2Signature,
	) -> ecdsa::Result<()> {
		let point = EncodedPoint::<Curve>::from_bytes(signature).unwrap();
		let pk = VerifyingKey::from_encoded_point(&point).unwrap();
		let sig = ecdsa::Signature::from_bytes(signature).unwrap();
		let t: BasicSigOf<Vec<u8>>;
		pk.verify_digest(digest, &sig)
	}
}*/

/*impl CandidType for L2Account {
	fn idl_serialize<S>(&self, serializer: S) -> core::result::Result<(), S::Error>
	where S: ic_cdk::export::candid::types::Serializer {
	}
}*/

impl Params {
	pub fn id(self: &Self) -> ChannelId {
		return ChannelId::default();
	}

	pub fn matches(self: &Self, state: &State) -> bool {
		return self.id() == state.channel && self.participants.len() == state.allocation.len();
	}
}

impl Funding {
	pub fn new(channel: ChannelId, participant: L2Account) -> Self {
		Self {
			channel: channel,
			participant: participant,
		}
	}
}
