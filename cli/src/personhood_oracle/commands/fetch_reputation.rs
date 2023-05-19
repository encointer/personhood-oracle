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
use my_node_runtime::AccountId;
use std::str::FromStr;
use substrate_api_client::GetStorage;

#[derive(Debug, Clone, Parser)]
pub struct FetchReputationCmd {
	pub account: String,
	pub cid: String,
	pub cindex: CeremonyIndexType,
}

impl FetchReputationCmd {
	pub fn run(&self, cli: &Cli) {
		let api = get_chain_api(&cli);
		let cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let cindex = self.cindex;
		let account = get_accountid_from_str(&self.account);

		let reputation = get_reputation(&api, &account, cid, cindex);

		println!("reputation for {} is: {:#?}", account, reputation);
	}
}

pub fn get_reputation(
	api: &ParentchainApi,
	prover: &AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) -> Reputation {
	api.get_storage_double_map(
		"EncointerCeremonies",
		"ParticipantReputation",
		(cid, cindex),
		prover.clone(),
		None,
	)
	.unwrap()
	.or(Some(Reputation::Unverified))
	.unwrap()
}
