extern crate icp_perun as perun;
use candid::{encode_args, Decode, Encode, Nat};
use garcon::Delay;
use ic_agent::{
	agent::http_transport::ReqwestHttpReplicaV2Transport, ic_types::Principal,
	identity::BasicIdentity, Agent, Identity,
};
use log::{error, info};
use perun::{test, types::Amount};
use ring::{rand::SystemRandom, signature::Ed25519KeyPair};
use std::{env, error::Error, result, time::Duration};

#[tokio::main]
async fn main() {
	pretty_env_logger::init();
	let (cid, url) = parse_args();

	if let Err(err) = deposit(cid, url).await {
		error!("{}", err);
	}
}

async fn deposit(
	cid: String,
	url: String,
) -> result::Result<(), Box<dyn Error + Sync + Send + 'static>> {
	let alice = 0;
	let canister_id = Principal::from_text(cid)?;
	let fid = test::Setup::new(123, false, false).funding(alice);
	let amount = Nat::from(111);

	let agent = Agent::builder()
		.with_transport(ReqwestHttpReplicaV2Transport::create(url)?)
		.with_identity(create_identity())
		.build()?;
	agent.fetch_root_key().await?; // Check for dev node.
	let waiter = Delay::builder()
		.throttle(Duration::from_millis(500))
		.timeout(Duration::from_secs(60 * 5))
		.build();

	for _ in 0..5 {
		// Deposit
		info!(
			"Depositing for channel: {} for peer IDx: {}, add: {} ICP",
			fid.channel, alice, amount
		);
		agent
			.update(&canister_id, "deposit")
			.with_arg(encode_args((&fid, &amount)).unwrap())
			.call_and_wait(waiter.clone())
			.await?;
		// Query deposit
		let response = agent
			.query(&canister_id, "query_deposit")
			.with_arg(Encode!(&fid).unwrap())
			.call()
			.await?;
		let res_amount = Decode!(&response, Option<Amount>)
			.unwrap()
			.unwrap_or_default();
		info!(
			"Querying for   channel: {} for peer IDx: {}, now: {} ICP",
			fid.channel, alice, res_amount
		);
	}
	Ok(())
}

fn parse_args() -> (String, String) {
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
	(cid, url)
}

fn create_identity() -> impl Identity {
	let rng = SystemRandom::new();
	let rng_data = Ed25519KeyPair::generate_pkcs8(&rng).expect("Could not generate a key pair.");

	BasicIdentity::from_key_pair(
		Ed25519KeyPair::from_pkcs8(rng_data.as_ref()).expect("Could not read the key pair."),
	)
}
