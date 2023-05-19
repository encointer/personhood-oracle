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
use crate::Cli;

#[derive(Debug, Clone, Parser)]
pub struct FetchNostrCmd {
	pub value: String,
}

impl FetchNostrCmd {
	pub fn run(&self, cli: &Cli) {}

	fn fetch_brenzi() {
		use nostr::prelude::*;
		use tungstenite::Message as WsMessage;

		// From https://gist.github.com/brenzi/32e9109599ea659a3d7277bfe2e8b9c7
		// Generate new random keys
		//let my_keys = Keys::generate();

		// or use your already existing
		//
		// From HEX or Bech32
		let my_keys =
			Keys::from_sk_str("nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r")
				.unwrap();

		// Show bech32 public key
		let bech32_pubkey: String = my_keys.public_key().to_bech32().unwrap();
		println!("Bech32 PubKey: {}", bech32_pubkey);
		println!("Secret key: {}", my_keys.secret_key().unwrap().to_bech32().unwrap());

		let metadata = Metadata::new()
			.name("somediddelidoo")
			.display_name("Some Diddelidoo")
			.about("I'm just testing");

		let event: Event = EventBuilder::set_metadata(metadata).to_event(&my_keys).unwrap();

		// New text note
		let event: Event = EventBuilder::new_text_note("Hello from Nostr SDK", &[])
			.to_event(&my_keys)
			.unwrap();

		// Connect to relay
		let (mut socket, _) =
			tungstenite::connect("wss://relay.damus.io").expect("Can't connect to relay");

		println!("sending text message with id {}", event.id.to_bech32().unwrap());

		// Send msg
		let msg = ClientMessage::new_event(event).as_json();
		socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");

		/*
			// create channel
			let metadata = Metadata::new()
				.name("diddelichannel")
				.about("I'm just testing")
				.picture(Url::parse("https://placekitten.com/200/200").unwrap());
			let event: Event = EventBuilder::new_channel(metadata).unwrap().to_event(&my_keys).unwrap();
			println!("creating channel with ID {}", event.id.to_bech32().unwrap());
			let msg = ClientMessage::new_event(event).as_json();

			socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
		*/
		let channel_id = ChannelId::from(
			EventId::from_bech32("note18kst54gwje8n5t3cfpdud4duwh37wtfu4zpefd6a6q24nc2uecqs6vy8lq")
				.unwrap(),
		);

		println!("posting a message to channel {}", channel_id);

		let event: Event = EventBuilder::new_channel_msg(
			channel_id,
			nostr::Url::parse("wss://relay.damus.io").unwrap(),
			"post in channel",
		)
		.to_event(&my_keys)
		.unwrap();

		let msg = ClientMessage::new_event(event).as_json();

		socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
	}
}
