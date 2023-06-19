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
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use itp_node_api::api_client::ParentchainApi;
use itp_types::H256;
use log::error;
use my_node_runtime::AccountId;
use std::str::FromStr;
use substrate_api_client::{GetStorage, ReadProof};

use itp_time_utils::{duration_now, now_as_secs, Duration};
use nostr::{
	nips::{
		nip58,
		nip58::{BadgeAward, BadgeDefinition},
	},
	prelude::{FromBech32, Secp256k1, XOnlyPublicKey},
	types::time::TimeSupplier,
	Keys, Tag, Timestamp,
};
#[derive(Debug, Clone, Parser)]
pub struct IssueNostrBadgeCmd {
	pub account: String,
	pub nostr_pub_key: String,
	pub cid: String,
	pub number_of_reputations: CeremonyIndexType,
}

impl IssueNostrBadgeCmd {
	pub fn run(&self, cli: &Cli) {
		let _api = get_chain_api(&cli);

		let badge_def = IssueNostrBadgeCmd::create_badge_def();

		let nostr_pub_key = XOnlyPublicKey::from_bech32(&self.nostr_pub_key).unwrap();
		let award = IssueNostrBadgeCmd::create_badge_award(badge_def, nostr_pub_key);
	}

	// Utility functions, will be moved

	fn get_time_supplier() -> DemoTimeSupplier {
		DemoTimeSupplier {}
	}

	fn get_ts() -> Timestamp {
		let now = duration_now();
		let time_supplier = IssueNostrBadgeCmd::get_time_supplier();
		let ts = time_supplier.to_timestamp(now);
		ts
	}
	fn create_badge_def() -> BadgeDefinition {
		// Just for demo purposes, should be reworked
		let builder = nip58::BadgeDefinitionBuilder::new("likely_person".to_owned());

		let secp = Secp256k1::new();
		let keys = Keys::generate_with_secp(&secp);
		let ts = IssueNostrBadgeCmd::get_ts();
		let def = builder.build(&keys, ts, &secp).unwrap();
		def
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

		let award = nip58::BadgeAward::new(&badge_definition_event, awarded_keys, &keys, ts, &secp)
			.unwrap();
		award
	}
	//fn create_badge() -> Option<nip58::ProfileBadgesEvent> {}
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
