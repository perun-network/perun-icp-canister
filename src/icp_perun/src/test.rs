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

use ed25519_dalek::{SecretKey, ExpandedSecretKey};
use crate::types::*;
use crate::CanisterState;
use candid::Encode;

#[derive(Default)]
pub struct Setup {
	pub parts: Vec<L2Account>,
	pub secrets: Vec<ExpandedSecretKey>,
	pub canister: CanisterState,
	pub params: Params,
	pub state: State,
}

pub fn keys(rand: u8, id: u8) -> (ExpandedSecretKey, L2Account) {
	let hash = Hash::digest(&[rand, id, 1, 2, 3]).0;
	let sk = SecretKey::from_bytes(&hash.as_slice()[..32]).unwrap();
	let esk = ExpandedSecretKey::from(&sk);
	let pk = L2Account((&sk).into());
	return (esk, pk)
}

impl Setup {
	pub fn new(rand: u8, finalized: bool, funded: bool) -> Self {
		let mut ret = Self::default();
		let key0 = keys(rand, 0);
		let key1 = keys(rand, 1);
		ret.parts = vec![key0.1, key1.1];
		ret.secrets = vec![key0.0, key1.0];

		ret.params.nonce = Hash::digest(&[rand, 0]);
		ret.params.participants = ret.parts.clone();

		ret.state.channel = ret.params.id();
		ret.state.version = (rand as u64) * 123;
		ret.state.allocation = vec![
			ret.params.nonce.0[0].into(),
			ret.params.nonce.0[1].into(),
		];
		ret.state.finalized = finalized;

		if funded {
			for (i, pk) in ret.parts.iter().enumerate() {
				ret.canister.deposit(
					Funding::new(ret.state.channel.clone(), pk.clone()),
					ret.state.allocation[i].clone()).unwrap();
			}
		}

		return ret
	}

	pub fn sign(&self) -> FullySignedState {
		self.sign_encoding(&Encode!(&self.state).unwrap())
	}
	pub fn sign_invalid(&self) -> FullySignedState {
		self.sign_encoding(&Encode!(&"invalid state").unwrap())
	}



	fn sign_encoding(&self, enc: &[u8]) -> FullySignedState {
		let mut state = FullySignedState::default();
		state.state = self.state.clone();
		for (i, key) in self.parts.iter().enumerate() {
			state.sigs.push(L2Signature(self.secrets[i].sign(&enc, &key.0).to_bytes().into()))
		}

		return state
	}
}