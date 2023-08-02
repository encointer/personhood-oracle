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
use codec::{Decode, Encode};
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use itc_rpc_client::direct_client::DirectApi;
use itp_node_api::api_client::ParentchainApi;
use itp_rpc::{RpcRequest, RpcResponse, RpcReturnValue};

use itp_types::DirectRequestStatus;
use itp_utils::FromHexPrefixed;
use log::error;

use std::str::FromStr;
use substrate_api_client::GetStorage;

#[derive(Debug, Clone, Parser)]
pub struct FetchReputationCmd {
	pub account: String,
	pub cid: String,
	pub number_of_reputations: CeremonyIndexType,
}

impl FetchReputationCmd {
	pub fn run(&self, cli: &Cli) {
		if let Ok(reputations) = self.fetch_reputation_rpc(cli) {
			let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();
			println!("reputation for {} is: {:#?}", &self.account, reputations);
			println!(
				"verified reputatations number: {} out of:{}",
				verified_reputations,
				reputations.len()
			);
		}
	}

	//TODO remove self argument, use user supplied `ProofOfAttendance` instead of fetching it
	//TODO see issue #26
	pub fn fetch_reputation_rpc(&self, cli: &Cli) -> Result<Vec<Reputation>, String> {
		let api = get_chain_api(cli);
		let direct_api = get_worker_api_direct(cli);
		let cindex = get_ceremony_index(&api);

		let cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let account = get_accountid_from_str(&self.account);

		let rpc_params = vec![
			cid.encode(),
			cindex.encode(),
			account.encode(),
			self.number_of_reputations.encode(),
		];
		println!("rpc_params is : {:#?}", &rpc_params);

		let rpc_params: Vec<String> = rpc_params
			.into_iter()
			.map(|p| (itp_utils::hex::hex_encode(p.as_slice())))
			.collect();

		let rpc_method = "personhoodoracle_fetchReputation".to_owned();
		let jsonrpc_call: String =
			RpcRequest::compose_jsonrpc_call(rpc_method, rpc_params).unwrap();

		let rpc_response_str = direct_api.get(&jsonrpc_call).unwrap();

		// Decode RPC response.
		let Ok(rpc_response) = serde_json::from_str::<RpcResponse>(&rpc_response_str) else {
			panic!("Can't parse RPC response: '{rpc_response_str}'");
		};
		println!("rpc_response is : {:#?}", &rpc_response);
		let rpc_return_value = match RpcReturnValue::from_hex(&rpc_response.result) {
			Ok(rpc_return_value) => rpc_return_value,
			Err(e) => panic!("Failed to decode RpcReturnValue: {:?}", e),
		};

		println!("rpc_return_value is : {:#?}", &rpc_return_value);

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
				let inner_error_msg: String =
					Decode::decode(&mut rpc_return_value.value.as_slice())
						.expect("Failed to decode reputations error msg");
				error!("inner_error_msg: {:#?}", &inner_error_msg);
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
