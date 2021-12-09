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

use crate::types::*;
use crate::CanisterState;
use candid::Encode;
use ed25519_dalek::{ExpandedSecretKey, SecretKey};

#[derive(Default)]
/// Contains a canister test environment with helper functions for easier
/// testing. Contains a canister, a set of channel participants, and a channel
/// state (along with matching channel parameters).
/// To test functionality, operate directly on the contained canister, and use
/// the setup's helper functions to generate the required arguments for the
/// canister calls.
pub struct Setup {
	pub parts: Vec<L2Account>,
	pub secrets: Vec<ExpandedSecretKey>,
	pub canister: CanisterState,
	pub params: Params,
	pub state: State,
}

/// Returns a default L1 account value.
pub fn default_account() -> L1Account {
	L1Account::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap()
}

/// Generates a public key pair from a randomness seed and an index.
fn keys(rand: u8, id: u8) -> (ExpandedSecretKey, L2Account) {
	let hash = Hash::digest(&[rand, id, 1, 2, 3]).0;
	let sk = SecretKey::from_bytes(&hash.as_slice()[..32]).unwrap();
	let esk = ExpandedSecretKey::from(&sk);
	let pk = L2Account((&sk).into());
	(esk, pk)
}

impl Setup {
	/// Creates a randomised test setup depending on the provided randomness
	/// seed. The `finalized` flag controls whether the generated channel state
	/// is final. The `funded` flag controls whether the outcome of the
	/// generated channel state should be deposited in the canister already.
	pub fn new(rand: u8, finalized: bool, funded: bool) -> Self {
		let mut ret = Self::default();
		let key0 = keys(rand, 0);
		let key1 = keys(rand, 1);
		ret.parts = vec![key0.1, key1.1];
		ret.secrets = vec![key0.0, key1.0];

		ret.params.nonce = Hash::digest(&[rand, 0]);
		ret.params.participants = ret.parts.clone();
		ret.params.challenge_duration = 1;

		ret.state.channel = ret.params.id();
		ret.state.version = (rand as u64) * 123;
		ret.state.allocation = vec![ret.params.nonce.0[0].into(), ret.params.nonce.0[1].into()];
		ret.state.finalized = finalized;

		if funded {
			for (i, _) in ret.parts.iter().enumerate() {
				ret.canister
					.deposit(ret.funding(i), ret.state.allocation[i].clone())
					.unwrap();
			}
		}

		ret
	}

	/// Signs the setup's channel state for all channel participants.
	pub fn sign_state(&self) -> FullySignedState {
		self.sign_encoding(&Encode!(&self.state).unwrap())
	}
	/// Creates a fully signed state with invalid signatures.
	pub fn sign_state_invalid(&self) -> FullySignedState {
		self.sign_encoding(&Encode!(&"invalid state").unwrap())
	}

	/// Returns the funding for a participant.
	pub fn funding(&self, part: usize) -> Funding {
		Funding::new(self.params.id(), self.parts[part].clone())
	}

	/// Creates a signed withdrawal request of the setup's channel for a given
	/// participant and receiver.
	pub fn withdrawal(&self, part: usize, receiver: L1Account) -> (WithdrawalRequest, L2Signature) {
		let funding = self.funding(part);
		let req = WithdrawalRequest::new(funding, receiver);
		(req.clone(), self.sign_withdrawal(&req, part))
	}

	/// Manually signs a withdrawal request using the requested participant's
	/// secret key.
	pub fn sign_withdrawal(&self, req: &WithdrawalRequest, part: usize) -> L2Signature {
		let enc = Encode!(req).unwrap();
		L2Signature(
			self.secrets[part]
				.sign(&enc, &self.parts[part].0)
				.to_bytes()
				.into(),
		)
	}

	/// Creates a fully signed state from the setup's state and uses the given
	/// byte encoding to generate its signatures.
	fn sign_encoding(&self, enc: &[u8]) -> FullySignedState {
		let mut state = FullySignedState::default();
		state.state = self.state.clone();
		for (i, key) in self.parts.iter().enumerate() {
			state.sigs.push(L2Signature(
				self.secrets[i].sign(enc, &key.0).to_bytes().into(),
			))
		}

		state
	}
}
