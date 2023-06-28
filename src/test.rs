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

use candid::Encode;
use ed25519_dalek::{ExpandedSecretKey, SecretKey};
use ic_cdk::export::Principal;
use oorandom::Rand64 as Prng;
use std::time::SystemTime;

use crate::{types::*, CanisterState};

/// Contains a canister test environment with helper functions for easier
/// testing. Contains a canister, a set of channel participants, and a channel
/// state (along with matching channel parameters).
/// To test functionality, operate directly on the contained canister, and use
/// the setup's helper functions to generate the required arguments for the
/// canister calls.
pub struct Setup {
	pub parts: Vec<L2Account>,
	pub secrets: Vec<ExpandedSecretKey>,
	pub canister: CanisterState<crate::icp::MockTXQuerier>,
	pub params: Params,
	pub state: State,
	pub prng: Prng,
}

/// Returns a default L1 account value.
pub fn default_account() -> L1Account {
	L1Account::from_text("bkyz2-fmaaa-aaaaa-qaaaq-cai").unwrap()
}

pub fn rand_hash(rng: &mut Prng) -> Hash {
	Hash::digest(&rng.rand_u64().to_ne_bytes())
}

/// Generates a public key pair from a randomness seed and an index.
fn rand_key(rand: &mut Prng) -> (ExpandedSecretKey, L2Account) {
	let bytes64: [u64; 4] = [
		rand.rand_u64(),
		rand.rand_u64(),
		rand.rand_u64(),
		rand.rand_u64(),
	];
	let bytes8: [u8; 32] = unsafe { std::mem::transmute(bytes64) };
	let sk = SecretKey::from_bytes(&bytes8).unwrap();
	let esk = ExpandedSecretKey::from(&sk);
	let pk = L2Account((&sk).into());
	(esk, pk)
}

static SEED_ENV_VAR: &str = "PERUN_TEST_SEED";

fn seed() -> u128 {
	let s = match std::env::var(SEED_ENV_VAR) {
		Ok(seed) => seed.parse().unwrap(),
		Err(_) => SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_nanos(),
	};
	println!("Using PRNG seed {}={}", SEED_ENV_VAR, s);
	s
}

impl Setup {
	pub fn new(finalized: bool, funded: bool) -> Self {
		Self::with_rng(Prng::new(seed()), finalized, funded)
	}

	/// Creates a randomised test setup depending on the provided randomness
	/// seed. The `finalized` flag controls whether the generated channel state
	/// is final. The `funded` flag controls whether the outcome of the
	/// generated channel state should be deposited in the canister already.
	pub fn with_rng(mut rand: Prng, finalized: bool, funded: bool) -> Self {
		let key0 = rand_key(&mut rand);
		let key1 = rand_key(&mut rand);

		let parts = vec![key0.1, key1.1];
		let secrets = vec![key0.0, key1.0];

		let params = Params {
			nonce: rand_hash(&mut rand),
			participants: parts.clone(),
			challenge_duration: 1,
		};

		let state = State {
			channel: params.id(),
			version: rand.rand_u64(),
			allocation: vec![
				(rand.rand_u64() >> 20).into(),
				(rand.rand_u64() >> 20).into(),
			],
			finalized,
		};

		let mut s = Setup {
			parts,
			secrets,
			canister: CanisterState::new(Default::default(), Principal::anonymous()),
			params,
			state,
			prng: rand,
		};

		if !funded {
			return s;
		}

		for (i, _) in s.parts.iter().enumerate() {
			s.canister
				.deposit(s.funding(i), s.state.allocation[i].clone())
				.unwrap();
		}
		s
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
	pub fn withdrawal_to(
		&self,
		part: usize,
		receiver: L1Account,
	) -> (WithdrawalTestRq, L2Signature) {
		let funding = self.funding(part);
		let req = WithdrawalTestRq::new(funding, receiver);
		(req.clone(), self.sign_withdrawal(&req, part))
	}

	/// Creates a signed withdrawal request with a preset receiver.
	pub fn withdrawal(&self, part: usize) -> (WithdrawalTestRq, L2Signature) {
		self.withdrawal_to(part, default_account())
	}

	/// Manually signs a withdrawal request using the requested participant's
	/// secret key.
	pub fn sign_withdrawal(&self, req: &WithdrawalTestRq, part: usize) -> L2Signature {
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
