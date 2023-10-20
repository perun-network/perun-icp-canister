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
// ic_types::Principal
use candid::{encode_args, Decode, Encode, Nat};
//use garcon::Delay;
//use ring::signature::Ed25519KeyPair;
//use ring::rand::SystemRandom;
use ic_agent::{
	agent::http_transport::ReqwestHttpReplicaV2Transport, identity::Secp256k1Identity, Agent,
	Identity,
};
use ic_cdk::export::Principal;
use ic_ledger_types::{
	AccountIdentifier, Memo, Tokens, TransferArgs, TransferResult, DEFAULT_SUBACCOUNT,
};
use icp_perun::{test, types::*};
use log::{error, info};
use std::{env, error, result::Result}; //, time::Duration

type Error = Box<dyn error::Error + Sync + Send + 'static>;

/// Holds all state for this demo.
struct Demo {
	pub setup: test::Setup,
	pub agent: Agent,
	pub canister: Principal,
	pub ledger: Principal,
}

/// Entry point for this example.
#[tokio::main]
async fn main() {
	pretty_env_logger::init();
	let (canister, ledger, url) = parse_args();

	if let Err(err) = walkthrough(canister, ledger, url).await {
		error!("{}", err);
	}
}

/// Walkthrough through the collaborative Perun protocol.
async fn walkthrough(cid: Principal, lid: Principal, url: String) -> Result<(), Error> {
	let mut demo = Demo::new(cid, lid, url, true).await?;
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
	info!("Demo finished successfully.");
	Ok(())
}

impl Demo {
	async fn new(
		canister: Principal,
		ledger: Principal,
		url: String,
		finalized: bool,
	) -> Result<Self, Error> {
		let agent = Agent::builder()
			.with_transport(ReqwestHttpReplicaV2Transport::create(url)?)
			.with_identity(create_identity())
			.build()?;
		agent.fetch_root_key().await?;
		let pri = agent.get_principal()?;
		println!("Generated Principal: {}", pri);

		Ok(Self {
			setup: test::Setup::new(finalized, false),
			agent,
			canister,
			ledger,
		})
	}

	async fn deposit(&self, amount: &Nat, part: usize) -> Result<(), Error> {
		let fid = self.setup.funding(part);
		info!(
			"Depositing       channel: {} for peer IDx: {}, add: {} ICP",
			fid.channel, part, amount
		);

		let mut amount_str = amount.to_string();
		amount_str.retain(|c| c != '_');
		let amount_u64 = amount_str.parse::<u64>().unwrap();

		let bytes = self
			.agent
			.update(&self.ledger, "transfer")
			.with_arg(
				Encode!(&TransferArgs {
					memo: Memo(fid.memo()),
					amount: Tokens::from_e8s(amount_u64),
					fee: Tokens::from_e8s(0),
					from_subaccount: None,
					to: AccountIdentifier::new(
						&L1Account::from_text(self.canister.to_string()).unwrap(),
						&DEFAULT_SUBACCOUNT
					),
					created_at_time: None,
				})
				.unwrap(),
			)
			.call_and_wait()
			.await?;
		let transfer_result = Some(Decode!(&bytes, TransferResult).unwrap());

		println!("transfer_result: {:?}", transfer_result);
		let block = transfer_result.unwrap().expect("transfer should not fail");
		info!("notifying canister of receipt {}", block);

		self.agent
			.update(&self.canister, "transaction_notification")
			.with_arg(Encode!(&block).unwrap())
			.call_and_wait()
			.await?;
		info!("triggering deposit");
		self.agent
			.update(&self.canister, "deposit")
			.with_arg(Encode!(&fid).unwrap())
			.call_and_wait()
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
			.update(&self.canister, "conclude_can") //
			.with_arg(encode_args((&self.setup.params, &sig_state)).unwrap())
			.call_and_wait()
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
		let (req, auth) = self.setup.withdrawal_to(
			part,
			L1Account::from_text(self.agent.get_principal().unwrap().to_string()).unwrap(),
		);
		self.agent
			.update(&self.canister, "withdraw_mocked")
			.with_arg(encode_args((&req, &auth)).unwrap())
			.call_and_wait()
			.await?;
		Ok(())
	}
}

/// First arg can be a ICP chain url, defaults to "http://localhost:4943/".
fn parse_args() -> (Principal, Principal, String) {
	let cid = env::var("ICP_PERUN_PRINCIPAL").expect("Need canister ID");
	let lid = env::var("ICP_LEDGER_PRINCIPAL").expect("Need ledger ID");
	let url = env::args()
		.skip(2)
		.next()
		.unwrap_or("http://localhost:4943/".into());
	info!("URL: {}", url);
	info!("Canister ID: {}", cid);
	info!("ICP ledger ID: {}", lid);
	(
		Principal::from_text(cid).unwrap(),
		Principal::from_text(lid).unwrap(),
		url,
	)
}

/// Loads a minter identity from a pem file.
fn create_identity() -> impl Identity {
	Secp256k1Identity::from_pem_file("./userdata/minter_identity.pem")
		.expect("loading default identity")
}
