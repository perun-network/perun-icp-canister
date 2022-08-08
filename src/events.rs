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

use async_trait::async_trait;
use ic_cdk::export::Principal;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::RwLock;

use crate::types::*;

lazy_static! {
	pub static ref STATE: RwLock<LocalEventRegisterer> = RwLock::new(LocalEventRegisterer::new());
}

/*
#[ic_cdk_macros::update]
async fn register_event(ch: ChannelId, time: Timestamp, e: Event) {
	STATE.write().unwrap().register_event(time, ch, e).await;
}
*/

#[ic_cdk_macros::query]
fn query_events(ch: ChannelId, start: Timestamp) -> Vec<Event> {
	STATE.read().unwrap().events_after(&ch, start)
}

#[derive(Clone, CandidType, Deserialize)]
pub enum Event {
	/// A participant supplied funds into the channel.
	Funded { who: L2Account, total: Amount },
	/// A dispute was started or refuted, along with the latest channel.
	Disputed(RegisteredState),
	/// Channel is now concluded and all funds can be withdrawn, no further updates are possible.
	Concluded,
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
