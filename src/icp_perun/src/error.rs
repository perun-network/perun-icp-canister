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

#[macro_export]
macro_rules! ensure {
	($cond:expr) => {
		if !($cond) {
			panic!("Error");
		}
	};
}

/// Contains all errors that can occur during an operation on the Perun
/// canister.
pub enum Error {
	/// Any kind of signature mismatch.
	Authentication,
	/// A non-finalized state was registered when a finalized state was
	/// expected.
	NotFinalized,
}

/// Canister operation result type.
pub type Result<T> = core::result::Result<T, Error>;