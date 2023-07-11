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
use nostr::{types::time::TimeSupplier, Event, Timestamp};

pub struct DemoTimeSupplier {}

impl TimeSupplier for DemoTimeSupplier {
	type Now = Duration;
	type StartingPoint = i64;

	fn instant_now(&self) -> <Self as TimeSupplier>::Now {
		duration_now()
	}
	fn now(&self) -> <Self as TimeSupplier>::StartingPoint {
		todo!()
	}
	fn duration_since_starting_point(
		&self,
		_: <Self as TimeSupplier>::StartingPoint,
	) -> std::time::Duration {
		todo!()
	}
	fn starting_point(&self) -> <Self as TimeSupplier>::StartingPoint {
		todo!()
	}
	fn elapsed_instant_since(
		&self,
		_: <Self as TimeSupplier>::Now,
		_: <Self as TimeSupplier>::Now,
	) -> std::time::Duration {
		todo!()
	}
	fn elapsed_since(
		&self,
		_: <Self as TimeSupplier>::StartingPoint,
		_: <Self as TimeSupplier>::StartingPoint,
	) -> Duration {
		todo!()
	}
	fn as_i64(&self, _value: Duration) -> i64 {
		now_as_secs() as i64
	}
	fn to_timestamp(&self, value: Duration) -> Timestamp {
		Timestamp::from(value.as_secs())
	}
}

pub fn get_time_supplier() -> DemoTimeSupplier {
	DemoTimeSupplier {}
}

pub fn get_ts() -> Timestamp {
	let now = duration_now();
	let time_supplier = get_time_supplier();
	let ts = time_supplier.to_timestamp(now);
	ts
}

pub fn send_nostr_events(events_to_send: Vec<Event>, relay: &str) {
	use nostr::prelude::*;
	use tungstenite_sgx as tungstenite;

	use nostr::{
		key::FromSkStr,
		nips::nip19::ToBech32,
		types::{Metadata as NostrMetadata, Timestamp as NostrTimestamp},
		ChannelId, ClientMessage, EventBuilder, EventId, Keys,
	};

	use tungstenite::Message as WsMessage;

	let secp = Secp256k1::new();

	// or use your already existing
	//
	// From HEX or Bech32
	let my_keys =
		Keys::from_sk_str("nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r", &secp)
			.unwrap();

	// Show bech32 public key
	let bech32_pubkey: String = my_keys.public_key().to_bech32().unwrap();
	println!("Bech32 PubKey: {}", bech32_pubkey);
	println!("Secret key: {}", my_keys.secret_key().unwrap().to_bech32().unwrap());

	// Connect to relay
	let (mut socket, response) =
        //tungstenite::connect("wss://nostr.lu.ke").expect("Can't connect to relay");
        tungstenite::connect(relay).expect("Can't connect to relay");

	println!("response is: {:#?}", &response);
	println!("socket is: {:#?}", &socket);

	for event in events_to_send {
		println!("sending text message with id {}", event.id.to_bech32().unwrap());

		let msg = ClientMessage::new_event(event).as_json();
		socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
	}
}
