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
	command_utils::{get_accountid_from_str, get_chain_api, get_worker_api_direct},
	Cli,
};
use codec::Decode;
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use itc_rpc_client::direct_client::DirectApi;
use itp_node_api::api_client::ParentchainApi;
use itp_rpc::{RpcRequest, RpcResponse, RpcReturnValue};
use itp_storage::{StorageHasher, StorageProof, StorageProofChecker};
use itp_types::{DirectRequestStatus, H256};
use itp_utils::FromHexPrefixed;
use log::error;

use my_node_runtime::AccountId;
use sp_runtime::traits::BlakeTwo256;
use std::str::FromStr;
use substrate_api_client::{GetStorage, ReadProof};

#[derive(Debug, Clone, Parser)]
pub struct FetchReputationCmd {
	pub account: String,
	pub cid: String,
	pub number_of_reputations: CeremonyIndexType,
}

impl FetchReputationCmd {
	pub fn run(&self, cli: &Cli) {
		let _api = get_chain_api(&cli);
		let _cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let _cindex = get_ceremony_index(&api);
		let _account = get_accountid_from_str(&self.account);

		let _direct_api = get_worker_api_direct(cli);

		if let Ok(reputation) = self.fetch_reputation_rpc(&cli) {
			todo!()
			// 	let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();
			// 	println!("reputation for {} is: {:#?}", account, reputations);
			// 	println!(
			// 		"verified reputatations number: {} out of:{}",
			// 		verified_reputations,
			// 		reputations.len()
			// 	);
			// 	println!("read proof is: {:#?}", read_proofs);
		}
	}

	//pub fn fetch_reputation_rpc(cli: &Cli) -> Result<Option<ReputationsWithReadProofs>, String> {
	pub fn fetch_reputation_rpc(&self, cli: &Cli) -> Result<Vec<Reputation>, String> {
		let api = get_chain_api(&cli);
		let direct_api = get_worker_api_direct(cli);
		let cindex = get_ceremony_index(&api);

		let rpc_params = vec![
			self.cid.to_string(),
			cindex.to_string(),
			self.account.to_string(),
			self.number_of_reputations.to_string(),
		];

		let rpc_params = rpc_params
			.into_iter()
			.map(|p| itp_utils::hex::hex_encode(p.as_bytes()))
			.collect();

		let rpc_method = "personhoodoracle_fetchReputation".to_owned();
		let jsonrpc_call: String =
			RpcRequest::compose_jsonrpc_call(rpc_method, rpc_params).unwrap();

		let rpc_response_str = direct_api.get(&jsonrpc_call).unwrap();

		// Decode RPC response.
		let Ok(rpc_response) = serde_json::from_str::<RpcResponse>(&rpc_response_str) else {
			panic!("Can't parse RPC response: '{rpc_response_str}'");
		};
		let rpc_return_value = match RpcReturnValue::from_hex(&rpc_response.result) {
			Ok(rpc_return_value) => rpc_return_value,
			Err(e) => panic!("Failed to decode RpcReturnValue: {:?}", e),
		};

		match rpc_return_value.status {
			DirectRequestStatus::Ok => {
				println!("Reputations fetched.");
				let reputations: Vec<Reputation> =
					Decode::decode(&mut rpc_return_value.value.as_slice())
						.expect("Failed to decode reputations");
				Ok(reputations)
			},
			_ => {
				let error_msg = "Reputations fetching failed";
				error!("{}", &error_msg);
				Err(error_msg.to_string())
			},
		}
	}
}

pub(crate) fn get_ceremony_index(api: &ParentchainApi) -> CeremonyIndexType {
	api.get_storage_value("EncointerScheduler", "CurrentCeremonyIndex", None)
		.unwrap()
		.unwrap()
}
