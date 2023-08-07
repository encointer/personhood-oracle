/*
	Copyright 2021 Integritee AG and Supercomputing Systems AG

	Licensed under the Apache License, Version 2.0 (the "License");
	you may not use this file except in compliance with the License.
	You may obtain a copy of the License at

		http://www.apache.org/licenses/LICENSE-2.0

	Unless required by applicable law or agreed to in writing, software
	distributed under the License is distributed on an "AS IS" BASIS,
	WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
	See the License for the specific language governing permissions and
	limitations under the License.

*/
use crate::{String, Vec};
use itp_time_utils::{duration_now, now_as_secs, Duration};
use log::info;
use nostr::{prelude::*, types::time::TimeProvider, Event, Timestamp};
use tungstenite_sgx as tungstenite;

use nostr::ClientMessage;

use tungstenite::Message as WsMessage;

pub struct DemoTimeProvider;

/// Time related operations are rather limited in `no_std` environments, which is why the `nostr`
/// crate provides a trait that can help `no_std` users have time related operations.
impl TimeProvider for DemoTimeProvider {
	type Now = Duration;
	type StartingPoint = i64;

	fn instant_now(&self) -> <Self as TimeProvider>::Now {
		duration_now()
	}
	fn now(&self) -> <Self as TimeProvider>::StartingPoint {
		unimplemented!()
	}
	fn duration_since_starting_point(
		&self,
		_: <Self as TimeProvider>::StartingPoint,
	) -> std::time::Duration {
		unimplemented!()
	}
	fn starting_point(&self) -> <Self as TimeProvider>::StartingPoint {
		unimplemented!()
	}
	fn elapsed_instant_since(
		&self,
		_: <Self as TimeProvider>::Now,
		_: <Self as TimeProvider>::Now,
	) -> std::time::Duration {
		unimplemented!()
	}
	fn elapsed_since(
		&self,
		_: <Self as TimeProvider>::StartingPoint,
		_: <Self as TimeProvider>::StartingPoint,
	) -> Duration {
		unimplemented!()
	}
	fn as_i64(&self, _value: Duration) -> i64 {
		now_as_secs() as i64
	}
	fn to_timestamp(&self, value: Duration) -> Timestamp {
		Timestamp::from(value.as_secs())
	}
}

pub fn get_time_provider() -> DemoTimeProvider {
	DemoTimeProvider {}
}

pub fn get_ts() -> Timestamp {
	let now = duration_now();
	let time_provider = get_time_provider();

	time_provider.to_timestamp(now)
}

pub fn send_nostr_events(events_to_send: Vec<Event>, relay: &str) -> Result<(), String> {
	// Connect to relay
	let (mut socket, _response) = tungstenite::connect(relay)
		.map_err(|e| format!("Can't connect to relay: error={:?}", e))?;

	for event in events_to_send {
		info!("sending text message with id {}", event.id.to_bech32().unwrap());

		let msg = ClientMessage::new_event(event).as_json();
		socket
			.write_message(WsMessage::Text(msg))
			.map_err(|e| format!("sendind nostr events failed: {:?}", e))?;
	}
	Ok(())
}
