//  Copyright 2022 PolyCrypt GmbH
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
use async_trait::async_trait;
use ic_cdk::export::Principal;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::RwLock;
lazy_static! {
	pub static ref STATE: RwLock<LocalEventRegisterer> = RwLock::new(LocalEventRegisterer::new());
}

#[ic_cdk_macros::update]
#[candid::candid_method]
async fn register_event(ch: ChannelId, time: Timestamp, e: Event) {
	STATE.write().unwrap().register_event(time, ch, e).await;
}

#[ic_cdk_macros::update]
#[candid::candid_method]
//async fn register_event_isolated(ch: ChannelId, time: Timestamp, e: Event) {
async fn register_event_isolated(regev: RegEvent) {
	// test event handling using this method
	let time = regev.time;
	let ch = regev.chanid;
	let e = regev.event;
	STATE.write().unwrap().register_event(time, ch, e).await;
}

// #[ic_cdk_macros::query]
// #[candid::candid_method(query)]
// fn query_events(ch: ChannelId, start: Timestamp) -> Vec<Event> {
// 	STATE.read().unwrap().events_after(&ch, start)
// }

// #[ic_cdk_macros::query]
// #[candid::candid_method(query)]
// fn query_events(et: ChannelTime) -> Vec<Event> {
// 	STATE.read().unwrap().events_after(&et.chanid, et.time)
// }
#[ic_cdk_macros::query]
#[candid::candid_method(query)]
fn query_events(et: ChannelTime) -> String {
	STATE.read().unwrap().events_after_str(&et.chanid, et.time)
}

#[derive(Clone, CandidType, Deserialize)]

pub enum Event {
	/// A participant supplied funds into the channel.
	Funded {
		who: L2Account,
		total: Amount,
		timestamp: Timestamp,
	},
	/// A dispute was started or refuted, along with the latest channel.
	Disputed {
		state: RegisteredState,
		timestamp: Timestamp,
	},
	/// Channel is now concluded and all funds can be withdrawn, no further updates are possible.
	Concluded {
		state: RegisteredState,
		timestamp: Timestamp,
	},
}

#[derive(PartialEq, Clone, Deserialize, Eq, Hash, CandidType)]

pub struct ChannelTime {
	/// The channel id.
	chanid: ChannelId,
	/// The time after which to return events.
	time: Timestamp,
}

#[derive(Clone, Deserialize, CandidType)]

pub struct RegEvent {
	/// The channel id.
	chanid: ChannelId,
	/// The time after which to return events.
	time: Timestamp,
	/// The event to register.
	event: Event,
}

#[async_trait]
pub trait EventRegisterer {
	async fn register_event(&mut self, time: Timestamp, ch: ChannelId, e: Event);
}

pub struct RPCEventRegisterer {
	event_canister: Principal,
}

#[async_trait]
impl EventRegisterer for RPCEventRegisterer {
	async fn register_event(&mut self, time: Timestamp, ch: ChannelId, e: Event) {
		let () = ic_cdk::call(self.event_canister, &"register_event", (ch, time, e))
			.await
			.unwrap();
	}
}

/// The event canister's state. Contains

pub struct CanisterState {
	perun_canister: Principal,
	imple: LocalEventRegisterer,
}

pub struct LocalEventRegisterer {
	/// All currently stored events.
	events: BTreeMap<ChannelId, BTreeMap<Timestamp, Vec<Event>>>,
}

#[async_trait]
impl EventRegisterer for LocalEventRegisterer {
	async fn register_event(&mut self, time: Timestamp, ch: ChannelId, e: Event) {
		let events = self.events.entry(ch).or_insert(Default::default());
		events.entry(time).or_insert(Default::default()).push(e);
	}
}

impl fmt::Display for State {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(
			f,
			"Channel: {} Version: {} Allocation: {:?} Finalized: {}",
			self.channel, self.version, self.allocation, self.finalized
		)
	}
}

impl fmt::Display for L2Account {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		// Use Debug implementation of PublicKey
		write!(f, "{:?}", self.0)
	}
}

impl fmt::Display for ChannelId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for byte in &self.0 {
			write!(f, "{:02x}", byte)?;
		}
		Ok(())
	}
}

impl fmt::Display for Event {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Event::Funded {
				who,
				total,
				timestamp,
			} => {
				write!(
					f,
					"Funded event: Funded_who={}, Funded_total=TotalStart{}TotalEnd, Funded_timestamp=TimestampStart{}TimestampEnd",
					who, total, timestamp
				)
			}
			Event::Disputed { state, timestamp } => {
				let alloc_string = state
					.state
					.allocation
					.iter()
					.map(|nat| format!("{}", nat))
					.collect::<Vec<String>>()
					.join(", ");

				write!(
					f,
					"Disputed event: Dispute_state=ChannelIDStart{}ChannelIDEnd, Dispute_state=VersionStart{}VersionEnd, Dispute_timeout=FinalizedStart{}FinalizedEnd, Dispute_alloc=AllocStart{}AllocEnd, Dispute_timeout=TimeoutStart{}TimeoutEnd, Dispute_timestamp=TimestampStart{}TimestampEnd",
					state.state.channel, state.state.version, state.state.finalized, alloc_string, state.timeout, timestamp
				)
			}

			Event::Concluded { state, timestamp } => {
				let alloc_string = state
					.state
					.allocation
					.iter()
					.map(|nat| format!("{}", nat))
					.collect::<Vec<String>>()
					.join(", ");
				write!(
					f,
					"Concluded event: Conclude_state=ChannelIDStart{}ChannelIDEnd, Conclude_state=VersionStart{}VersionEnd, Conclude_timeout=FinalizedStart{}FinalizedEnd, Conclude_alloc=AllocStart{}AllocEnd, Conclude_timeout=TimeoutStart{}TimeoutEnd, Conclude_timestamp=TimestampStart{}TimestampEnd",
					state.state.channel, state.state.version, state.state.finalized, alloc_string, state.timeout, timestamp
				)
			}
		}
	}
}

#[async_trait]
impl EventRegisterer for CanisterState {
	async fn register_event(&mut self, time: Timestamp, ch: ChannelId, e: Event) {
		if ic_cdk::api::caller() != self.perun_canister {
			return;
		}
		self.imple.register_event(time, ch, e).await;
	}
}

impl LocalEventRegisterer {
	pub fn events_after(&self, ch: &ChannelId, time: Timestamp) -> Vec<Event> {
		self.events.get(ch).map_or(vec![], |events| {
			let mut ret = vec![];
			for (_, es) in events.range(time..) {
				ret.extend(es.iter().cloned());
			}
			ret
		})
	}

	pub fn events_after_str(&self, ch: &ChannelId, time: Timestamp) -> String {
		self.events
			.get(ch)
			.map_or(String::from("No events"), |events| {
				let mut ret = String::new();
				for (_, es) in events.range(time..) {
					for e in es {
						ret.push_str(&format!("{}\n", e));
					}
				}
				ret
			})
	}

	pub fn gc(&mut self, min_time: Timestamp) {
		for (_, ch_events) in self.events.iter_mut() {
			ch_events.retain(|&t, _| t >= min_time);
		}
		self.events.retain(|_, events| !events.is_empty())
	}

	pub fn new() -> Self {
		Self {
			events: Default::default(),
		}
	}
}

impl CanisterState {
	pub fn new(perun_canister: Principal) -> Self {
		Self {
			perun_canister: perun_canister,
			imple: LocalEventRegisterer::new(),
		}
	}

	pub fn events_after(&self, ch: &ChannelId, time: Timestamp) -> Vec<Event> {
		self.imple.events_after(ch, time)
	}
}
