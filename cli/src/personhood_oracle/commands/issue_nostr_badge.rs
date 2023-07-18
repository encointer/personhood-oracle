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
	command_utils::{get_chain_api, get_worker_api_direct},
	Cli,
};
use encointer_primitives::scheduler::CeremonyIndexType;
use itc_rpc_client::direct_client::DirectApi;
use itp_rpc::{RpcRequest, RpcResponse, RpcReturnValue};

use itp_utils::FromHexPrefixed;

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
		//todo!();
		let api = get_chain_api(cli);
		let direct_api = get_worker_api_direct(cli);
		let cindex = get_ceremony_index(&api);

		let rpc_params = vec![
			self.cid.to_string(),
			cindex.to_string(),
			self.account.to_string(),
			self.number_of_reputations.to_string(),
			self.relay.clone(),
		];

		let rpc_params = rpc_params
			.into_iter()
			.map(|p| itp_utils::hex::hex_encode(p.as_bytes()))
			.collect();

		let rpc_method = "personhoodoracle_issueNostrBadge".to_owned();
		let jsonrpc_call: String =
			RpcRequest::compose_jsonrpc_call(rpc_method, rpc_params).unwrap();

		let rpc_response_str_result = direct_api.get(&jsonrpc_call);
		println!("rpc_response_str_result is:{:#?}", &rpc_response_str_result);
		let rpc_response_str = rpc_response_str_result.unwrap();

		// Decode RPC response.
		let Ok(rpc_response) = serde_json::from_str::<RpcResponse>(&rpc_response_str) else {
			panic!("Can't parse RPC response: '{rpc_response_str}'");
		};
		let _rpc_return_value = match RpcReturnValue::from_hex(&rpc_response.result) {
			Ok(rpc_return_value) => rpc_return_value,
			Err(e) => panic!("Failed to decode RpcReturnValue: {:?}", e),
		};
	}
}
