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

use ic_cdk::export::candid::{CandidType, Deserialize};

#[macro_export]
macro_rules! require {
	($cond:expr, $err:ident) => {
		if !($cond) {
			return Err(Error::$err);
		}
	};
	($cond:expr, $err:expr) => {
		if !($cond) {
			return Err($err);
		}
	};
}

#[derive(PartialEq, Eq, CandidType, Deserialize, Debug)]
/// Contains all errors that can occur during an operation on the Perun
/// canister.
pub enum Error {
	/// Any kind of signature mismatch.
	Authentication,
	/// A non-finalized state was registered when a finalized state was
	/// expected.
	NotFinalized,
	/// A channel has been concluded or disputed after conclusion.
	AlreadyConcluded,
	/// In some way, the input was invalid.
	InvalidInput,
	/// When trying get more funds out of a channel than have been put into it.
	InsufficientFunding,
	/// When a state that is registered for dispute is older than the previously
	/// registered state.
	OutdatedState,
	/// Error while interaction with the ledger.
	LedgerError,
	/// Error receiving ICP tokens.
	ReceiverError(crate::icp::ICPReceiverError),
}
impl std::fmt::Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		std::fmt::Debug::fmt(self, f)
	}
}
/// Canister operation result type.
pub type Result<T> = core::result::Result<T, Error>;
