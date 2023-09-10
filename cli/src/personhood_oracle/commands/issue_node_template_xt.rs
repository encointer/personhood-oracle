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
use encointer_primitives::communities::CommunityIdentifier;
use itc_rpc_client::direct_client::DirectApi;
use itp_rpc::{RpcRequest, RpcResponse, RpcReturnValue};
use itp_types::DirectRequestStatus;
use itp_utils::FromHexPrefixed;
use log::*;
use std::str::FromStr;

#[derive(Debug, Clone, Parser)]
pub struct IssueNodeTemplateXtCmd {
	pub account: String,
	pub node_template_idx: u32,
	pub cid: String,
	// TODO add proofs
	// pub proofs: Vec<ProofOfAttendance>
}

use crate::personhood_oracle::commands::fetch_reputation::get_ceremony_index;

impl IssueNodeTemplateXtCmd {
	pub fn run(&self, cli: &Cli) {
		//todo!();
		let api = get_chain_api(cli);
		let direct_api = get_worker_api_direct(cli);
		let cindex = get_ceremony_index(&api);

		let cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let account = get_accountid_from_str(&self.account);
		let node_template_idx = self.node_template_idx;

		let rpc_params =
			vec![cid.encode(), cindex.encode(), account.encode(), node_template_idx.encode()];
		trace!("rpc_params is : {:?}", &rpc_params);

		let rpc_params = rpc_params
			.into_iter()
			.map(|p| itp_utils::hex::hex_encode(p.as_slice()))
			.collect();

		let rpc_method = "personhoodoracle_issueNodeTemplateXt".to_owned();
		let jsonrpc_call: String =
			RpcRequest::compose_jsonrpc_call(rpc_method, rpc_params).unwrap();

		let rpc_response_str_result = direct_api.get(&jsonrpc_call);
		debug!("rpc_response_str_result is:{:?}", &rpc_response_str_result);
		let rpc_response_str = rpc_response_str_result.unwrap();

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
				println!("NodeTemplate Idex has been issued successfully.");
			},
			_ => {
				let error_msg = "Node Template Xt issuance failed";
				error!("{}", &error_msg);
				let inner_error_msg: String =
					Decode::decode(&mut rpc_return_value.value.as_slice())
						.expect("Failed to decode node tempölate xt issuing RPC error msg");
				error!("Node Template issuing failed: {:#?}", &inner_error_msg);
			},
		}
	}
}
