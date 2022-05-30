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

use candid::{encode_args, Decode, Encode, Nat};
use garcon::Delay;
use ic_agent::{
	agent::http_transport::ReqwestHttpReplicaV2Transport, ic_types::Principal,
	identity::BasicIdentity, Agent, Identity,
};
use icp_perun::{test, types::*};
use log::{error, info};
use ring::{rand::SystemRandom, signature::Ed25519KeyPair};
use std::{env, error, result::Result, time::Duration};

type Error = Box<dyn error::Error + Sync + Send + 'static>;

/// Holds all state for this demo.
struct Demo {
	pub setup: test::Setup,
	pub agent: Agent,
	pub canister: Principal,
	pub delay: Delay,
}

/// Entry point for this example.
#[tokio::main]
async fn main() {
	pretty_env_logger::init();
	let (canister, url) = parse_args();

	if let Err(err) = walkthrough(canister, url).await {
		error!("{}", err);
	}
}

/// Walkthrough through the collaborative Perun protocol.
async fn walkthrough(cid: Principal, url: String) -> Result<(), Error> {
	let mut demo = Demo::new(cid, url, true).await?;
	let (alice, bob) = (0, 1);

	// Query on-chain balances.
	demo.query_holdings(alice).await?;
	demo.query_holdings(bob).await?;
	// Deposit for Alice and Bob.
	demo.deposit(&demo.setup.state.allocation[alice], alice)
		.await?;
	demo.deposit(&demo.setup.state.allocation[bob], bob).await?;
	// Query on-chain balances.
	demo.query_holdings(alice).await?;
	demo.query_holdings(bob).await?;
	// Update off-chain balances.
	demo.setup.state.allocation.swap(alice, bob);
	// Conclude the channel.
	demo.conclude().await?;
	let state = demo.query_state().await?.unwrap();
	info!("state is final: {}", state.state.finalized);
	// Withdraw balances.
	demo.withdraw(alice).await?;
	demo.withdraw(bob).await?;
	// Query on-chain balances.
	demo.query_holdings(alice).await?;
	demo.query_holdings(bob).await?;

	Ok(())
}

impl Demo {
	async fn new(canister: Principal, url: String, finalized: bool) -> Result<Self, Error> {
		let agent = Agent::builder()
			.with_transport(ReqwestHttpReplicaV2Transport::create(url)?)
			.with_identity(create_identity())
			.build()?;
		agent.fetch_root_key().await?; // Check for dev node.
		let delay = Delay::builder()
			.throttle(Duration::from_millis(500))
			.timeout(Duration::from_secs(60 * 5))
			.build();

		Ok(Self {
			setup: test::Setup::new(finalized, false),
			agent,
			canister,
			delay,
		})
	}

	async fn deposit(&self, amount: &Nat, part: usize) -> Result<(), Error> {
		let fid = self.setup.funding(part);
		info!(
			"Depositing       channel: {} for peer IDx: {}, add: {} ICP",
			fid.channel, part, amount
		);
		self.agent
			.update(&self.canister, "deposit_mocked")
			.with_arg(encode_args((&fid, &amount)).unwrap())
			.call_and_wait(self.delay.clone())
			.await?;
		Ok(())
	}

	async fn query_holdings(&self, part: usize) -> Result<(), Error> {
		let fid = self.setup.funding(part);
		let response = self
			.agent
			.query(&self.canister, "query_holdings")
			.with_arg(Encode!(&fid).unwrap())
			.call()
			.await?;
		let res_amount = Decode!(&response, Option<Amount>)
			.unwrap()
			.unwrap_or_default();
		info!(
			"Querying deposit channel: {} for peer IDx: {}, now: {} ICP",
			fid.channel, part, res_amount
		);
		Ok(())
	}

	async fn query_state(&self) -> Result<Option<RegisteredState>, Error> {
		let response = self
			.agent
			.query(&self.canister, "query_state")
			.with_arg(Encode!(&self.setup.state.channel).unwrap())
			.call()
			.await?;
		Ok(Decode!(&response, Option<RegisteredState>).unwrap())
	}

	async fn conclude(&self) -> Result<(), Error> {
		info!("Concluding       channel: {}", self.setup.params.id());
		let sig_state = self.setup.sign_state();
		self.agent
			.update(&self.canister, "conclude")
			.with_arg(encode_args((&self.setup.params, &sig_state)).unwrap())
			.call_and_wait(self.delay.clone())
			.await?;
		Ok(())
	}

	async fn withdraw(&self, part: usize) -> Result<(), Error> {
		info!(
			"Withdrawing      channel: {} for peer IDx: {}",
			self.setup.params.id(),
			part
		);
		// Use the Canister ID here as receiver since the funds are currently mocked.
		let (req, auth) = self.setup.withdrawal_to(part, self.canister);
		self.agent
			.update(&self.canister, "withdraw_mocked")
			.with_arg(encode_args((&req, &auth)).unwrap())
			.call_and_wait(self.delay.clone())
			.await?;
		Ok(())
	}
}

/// First arg must be a canister ID, eg: "rrkah-fqaaa-aaaaa-aaaaq-cai".
/// Second arg can be a ICP chain url, defaults to "http://localhost:8000/".
fn parse_args() -> (Principal, String) {
	let cid = env::args()
		.skip(1)
		.next()
		.expect("Need canister ID as first arg");
	let url = env::args()
		.skip(2)
		.next()
		.unwrap_or("http://localhost:8000/".into());
	info!("URL: {}", url);
	info!("Canister ID: {}", cid);
	(Principal::from_text(cid).unwrap(), url)
}

/// Creates a random on-chain identity for making calls.
fn create_identity() -> impl Identity {
	let rng = SystemRandom::new();
	let rng_data = Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");

	BasicIdentity::from_key_pair(
		Ed25519KeyPair::from_pkcs8(rng_data.as_ref()).expect("Could not read the key pair."),
	)
}
