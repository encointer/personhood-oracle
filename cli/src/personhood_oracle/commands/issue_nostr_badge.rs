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

use crate::{
	command_utils::{get_accountid_from_str, get_chain_api},
	Cli,
};
use encointer_primitives::{communities::CommunityIdentifier, scheduler::CeremonyIndexType};

use std::str::FromStr;

use crate::personhood_oracle::FetchReputationCmd;
use itp_time_utils::{duration_now, now_as_secs, Duration};
use nostr::{
	key::FromSkStr,
	nips::{
		nip58,
		nip58::{BadgeAward, BadgeDefinition, ImageDimensions},
	},
	prelude::{FromBech32, Secp256k1, XOnlyPublicKey},
	types::time::TimeSupplier,
	Event, Keys, Tag, Timestamp,
};
#[derive(Debug, Clone, Parser)]
pub struct IssueNostrBadgeCmd {
	pub account: String,
	pub nostr_pub_key: String,
	pub cid: String,
	pub number_of_reputations: CeremonyIndexType,
	pub relay: String,
	// TODO add proofs
	// pub proofs: Vec<ProofOfAttendance>
}

use crate::personhood_oracle::commands::fetch_reputation::get_ceremony_index;

impl IssueNostrBadgeCmd {
	pub fn run(&self, cli: &Cli) {
		let api = get_chain_api(cli);
		let cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let cindex = get_ceremony_index(&api);
		let account = get_accountid_from_str(&self.account);

		if let Some((reputations, read_proofs)) = FetchReputationCmd::fetch_reputation(
			&api,
			cid,
			cindex,
			account.clone(),
			self.number_of_reputations,
		) {
			let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();
			println!("reputation for {} is: {:#?}", account, reputations);
			println!(
				"verified reputatations number: {} out of:{}",
				verified_reputations,
				reputations.len()
			);
			println!("read proof is: {:#?}", read_proofs);
			// At this point the following should be true:
			// The reputatuion is valid and
			// The reputatuion has been proof read and
			let badge_def = IssueNostrBadgeCmd::create_badge_def();

			let nostr_pub_key = XOnlyPublicKey::from_bech32(&self.nostr_pub_key).unwrap();
			let award = IssueNostrBadgeCmd::create_badge_award(badge_def.clone(), nostr_pub_key);

			let badge_def = badge_def.into_event();
			let award = award.into_event();

			println!("badge_def struct is: {:#?}", badge_def);
			println!("badge_def as json: {:#?}", badge_def.as_json());

			Self::send_nostr_events(vec![badge_def, award], &self.relay)
			// The reputation is consumed for this purpose HERE, after the nostr badge has been issued successfully.
		}
	}

	// Utility functions, will be moved

	fn get_time_supplier() -> DemoTimeSupplier {
		DemoTimeSupplier {}
	}

	fn get_ts() -> Timestamp {
		let now = duration_now();
		let time_supplier = IssueNostrBadgeCmd::get_time_supplier();

		time_supplier.to_timestamp(now)
	}
	fn create_badge_def() -> BadgeDefinition {
		// Just for demo purposes, should be reworked
		let builder = nip58::BadgeDefinitionBuilder::new("likely_person".to_owned());
		let thumb_size = ImageDimensions(181, 151);
		let thumbs = vec![
			(
				"https://parachains.info/images/parachains/1625163231_encointer_logo.png"
					.to_owned(),
				Some(thumb_size),
			),
			(
				"https://parachains.info/images/parachains/1625163231_encointer_logo.png"
					.to_owned(),
				None,
			),
		];
		let builder = builder
			.image(
				"https://parachains.info/images/parachains/1625163231_encointer_logo.png"
					.to_owned(),
			)
			.thumbs(thumbs)
			.image_dimensions(ImageDimensions(181, 151));

		let secp = Secp256k1::new();
		//let keys = Keys::generate_with_secp(&secp);\
		let keys = Keys::from_sk_str(
			"nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r",
			&secp,
		)
		.unwrap();
		let ts = IssueNostrBadgeCmd::get_ts();

		builder.build(&keys, ts, &secp).unwrap()
	}
	fn create_badge_award(
		badge_definition: BadgeDefinition,
		awarded_pub_key: XOnlyPublicKey,
	) -> BadgeAward {
		let badge_definition_event = badge_definition.into_event();
		let awarded_keys = vec![Tag::PubKey(awarded_pub_key, None)];

		let secp = Secp256k1::new();
		let keys = Keys::generate_with_secp(&secp);
		let ts = IssueNostrBadgeCmd::get_ts();

		nip58::BadgeAward::new(&badge_definition_event, awarded_keys, &keys, ts, &secp).unwrap()
	}
	//fn create_badge() -> Option<nip58::ProfileBadgesEvent> {}

	fn send_nostr_events(events_to_send: Vec<Event>, relay: &str) {
		use nostr::prelude::*;
		//use tungstenite_sgx as tungstenite;

		use nostr::{key::FromSkStr, nips::nip19::ToBech32, ClientMessage, Keys};

		use tungstenite::Message as WsMessage;

		let secp = Secp256k1::new();

		// or use your already existing
		//
		// From HEX or Bech32
		let my_keys = Keys::from_sk_str(
			"nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r",
			&secp,
		)
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
}

struct DemoTimeSupplier {}

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
