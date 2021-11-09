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

use ed25519_dalek::{SecretKey, PublicKey, ExpandedSecretKey};
use crate::types::*;
use crate::CanisterState;

static SECRET_KEY_BYTES: [u8; 32] = [
	157, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068, 073, 197,
	105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
];

pub fn alice_keys() -> (ExpandedSecretKey, L2Account) {
	let alice_sk = SecretKey::from_bytes(&SECRET_KEY_BYTES).unwrap();
	let alice_esk = ExpandedSecretKey::from(&alice_sk);
	let alice_pk: PublicKey = (&alice_sk).into();
	let alice = L2Account(alice_pk);
	return (alice_esk, alice);
}

pub fn setup() -> (CanisterState, ExpandedSecretKey, L2Account) {
	let (esk, pk) = alice_keys();
	return (CanisterState::default(), esk, pk);
}
