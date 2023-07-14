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
	attestation::{
		generate_dcap_ra_extrinsic_from_quote_internal,
		generate_ias_ra_extrinsic_from_der_cert_internal,
	},
	rpc::{
		encointer_utils::fetch_reputation,
		nostr_utils::{get_ts, send_nostr_events},
	},
	utils::get_validator_accessor_from_solo_or_parachain,
};
use codec::Encode;
use core::{result::Result, str::FromStr};
use encointer_primitives::{ceremonies::Reputation, communities::CommunityIdentifier};
use ita_sgx_runtime::Runtime;
use itc_parentchain::light_client::{concurrent_access::ValidatorAccess, ExtrinsicSender};
use itp_primitives_cache::{GetPrimitives, GLOBAL_PRIMITIVES_CACHE};
use itp_rpc::RpcReturnValue;
use itp_sgx_crypto::key_repository::AccessPubkey;
use itp_stf_executor::getter_executor::ExecuteGetter;
use itp_stf_primitives::types::AccountId;
use itp_top_pool_author::traits::AuthorApi;
use itp_types::{DirectRequestStatus, Request, ShardIdentifier, H256};
use itp_utils::{FromHexPrefixed, ToHexPrefixed};
use its_primitives::types::block::SignedBlock;
use its_sidechain::rpc_handler::{direct_top_pool_api, import_block_api};
use jsonrpc_core::{serde_json::json, IoHandler, Params, Value};
use nostr::{
	key::FromSkStr,
	nips::{
		nip58,
		nip58::{BadgeAward, BadgeDefinition, ImageDimensions},
	},
	prelude::{FromBech32, Secp256k1, XOnlyPublicKey},
	Keys, Tag,
};
use sgx_crypto_helper::rsa3072::Rsa3072PubKey;
use sp_runtime::OpaqueExtrinsic;
use std::{
	borrow::ToOwned,
	format, str,
	string::{String, ToString},
	sync::Arc,
	vec::Vec,
};

fn compute_hex_encoded_return_error(error_msg: &str) -> String {
	RpcReturnValue::from_error_message(error_msg).to_hex()
}

fn get_all_rpc_methods_string(io_handler: &IoHandler) -> String {
	let method_string = io_handler
		.iter()
		.map(|rp_tuple| rp_tuple.0.to_owned())
		.collect::<Vec<String>>()
		.join(", ");

	format!("methods: [{}]", method_string)
}

pub fn public_api_rpc_handler<Author, GetterExecutor, AccessShieldingKey>(
	top_pool_author: Arc<Author>,
	getter_executor: Arc<GetterExecutor>,
	shielding_key: Arc<AccessShieldingKey>,
) -> IoHandler
where
	Author: AuthorApi<H256, H256> + Send + Sync + 'static,
	GetterExecutor: ExecuteGetter + Send + Sync + 'static,
	AccessShieldingKey: AccessPubkey<KeyType = Rsa3072PubKey> + Send + Sync + 'static,
{
	let io = IoHandler::new();

	// Add direct TOP pool rpc methods
	let mut io = direct_top_pool_api::add_top_pool_direct_rpc_methods(top_pool_author, io);

	// author_getShieldingKey
	let rsa_pubkey_name: &str = "author_getShieldingKey";
	io.add_sync_method(rsa_pubkey_name, move |_: Params| {
		let rsa_pubkey = match shielding_key.retrieve_pubkey() {
			Ok(key) => key,
			Err(status) => {
				let error_msg: String = format!("Could not get rsa pubkey due to: {}", status);
				return Ok(json!(compute_hex_encoded_return_error(error_msg.as_str())))
			},
		};

		let rsa_pubkey_json = match serde_json::to_string(&rsa_pubkey) {
			Ok(k) => k,
			Err(x) => {
				let error_msg: String =
					format!("[Enclave] can't serialize rsa_pubkey {:?} {}", rsa_pubkey, x);
				return Ok(json!(compute_hex_encoded_return_error(error_msg.as_str())))
			},
		};
		let json_value =
			RpcReturnValue::new(rsa_pubkey_json.encode(), false, DirectRequestStatus::Ok);
		Ok(json!(json_value.to_hex()))
	});

	let mu_ra_url_name: &str = "author_getMuRaUrl";
	io.add_sync_method(mu_ra_url_name, move |_: Params| {
		let url = match GLOBAL_PRIMITIVES_CACHE.get_mu_ra_url() {
			Ok(url) => url,
			Err(status) => {
				let error_msg: String = format!("Could not get mu ra url due to: {}", status);
				return Ok(json!(compute_hex_encoded_return_error(error_msg.as_str())))
			},
		};

		let json_value = RpcReturnValue::new(url.encode(), false, DirectRequestStatus::Ok);
		Ok(json!(json_value.to_hex()))
	});

	let untrusted_url_name: &str = "author_getUntrustedUrl";
	io.add_sync_method(untrusted_url_name, move |_: Params| {
		let url = match GLOBAL_PRIMITIVES_CACHE.get_untrusted_worker_url() {
			Ok(url) => url,
			Err(status) => {
				let error_msg: String = format!("Could not get untrusted url due to: {}", status);
				return Ok(json!(compute_hex_encoded_return_error(error_msg.as_str())))
			},
		};

		let json_value = RpcReturnValue::new(url.encode(), false, DirectRequestStatus::Ok);
		Ok(json!(json_value.to_hex()))
	});

	// chain_subscribeAllHeads
	let chain_subscribe_all_heads_name: &str = "chain_subscribeAllHeads";
	io.add_sync_method(chain_subscribe_all_heads_name, |_: Params| {
		let parsed = "world";
		Ok(Value::String(format!("hello, {}", parsed)))
	});

	// state_getMetadata
	let state_get_metadata_name: &str = "state_getMetadata";
	io.add_sync_method(state_get_metadata_name, |_: Params| {
		let metadata = Runtime::metadata();
		let json_value = RpcReturnValue::new(metadata.into(), false, DirectRequestStatus::Ok);
		Ok(json!(json_value.to_hex()))
	});

	// state_getRuntimeVersion
	let state_get_runtime_version_name: &str = "state_getRuntimeVersion";
	io.add_sync_method(state_get_runtime_version_name, |_: Params| {
		let parsed = "world";
		Ok(Value::String(format!("hello, {}", parsed)))
	});

	// state_executeGetter
	let state_execute_getter_name: &str = "state_executeGetter";
	io.add_sync_method(state_execute_getter_name, move |params: Params| {
		let json_value = match execute_getter_inner(getter_executor.as_ref(), params) {
			Ok(state_getter_value) => RpcReturnValue {
				do_watch: false,
				value: state_getter_value.encode(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};
		Ok(json!(json_value))
	});

	// attesteer_forward_dcap_quote
	let attesteer_forward_dcap_quote: &str = "attesteer_forwardDcapQuote";
	io.add_sync_method(attesteer_forward_dcap_quote, move |params: Params| {
		let json_value = match forward_dcap_quote_inner(params) {
			Ok(val) => RpcReturnValue {
				do_watch: false,
				value: val.encode(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};

		Ok(json!(json_value))
	});

	// attesteer_forward_ias_attestation_report
	let attesteer_forward_ias_attestation_report: &str = "attesteer_forwardIasAttestationReport";
	io.add_sync_method(attesteer_forward_ias_attestation_report, move |params: Params| {
		let json_value = match attesteer_forward_ias_attestation_report_inner(params) {
			Ok(val) => RpcReturnValue {
				do_watch: false,
				value: val.encode(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};

		Ok(json!(json_value))
	});

	// personhoodoracle_issueNostrBadge
	let personhoodoracle_issue_nostr_badge: &str = "personhoodoracle_issueNostrBadge";
	io.add_sync_method(personhoodoracle_issue_nostr_badge, move |params: Params| {
		let json_value = match issue_nostr_badge_inner(params) {
			Ok(val) => RpcReturnValue {
				do_watch: false,
				value: val.encode(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};

		Ok(json!(json_value))
	});

	// personhoodoracle_fetchReputation
	let personhoodoracle_fetch_reputation: &str = "personhoodoracle_fetchReputation";
	io.add_sync_method(personhoodoracle_fetch_reputation, move |params: Params| {
		let json_value = match fetch_reputation_inner(params) {
			Ok(val) => RpcReturnValue {
				do_watch: false,
				value: val.encode(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};

		Ok(json!(json_value))
	});

	// system_health
	let state_health_name: &str = "system_health";
	io.add_sync_method(state_health_name, |_: Params| {
		let parsed = "world";
		Ok(Value::String(format!("hello, {}", parsed)))
	});

	// system_name
	let state_name_name: &str = "system_name";
	io.add_sync_method(state_name_name, |_: Params| {
		let parsed = "world";
		Ok(Value::String(format!("hello, {}", parsed)))
	});

	// system_version
	let state_version_name: &str = "system_version";
	io.add_sync_method(state_version_name, |_: Params| {
		let parsed = "world";
		Ok(Value::String(format!("hello, {}", parsed)))
	});

	// returns all rpcs methods
	let rpc_methods_string = get_all_rpc_methods_string(&io);
	io.add_sync_method("rpc_methods", move |_: Params| {
		Ok(Value::String(rpc_methods_string.to_owned()))
	});

	io
}

fn execute_getter_inner<G: ExecuteGetter>(
	getter_executor: &G,
	params: Params,
) -> Result<Option<Vec<u8>>, String> {
	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	let request =
		Request::from_hex(&hex_encoded_params[0].clone()).map_err(|e| format!("{:?}", e))?;

	let shard: ShardIdentifier = request.shard;
	let encoded_trusted_getter: Vec<u8> = request.cyphertext;

	let getter_result = getter_executor
		.execute_getter(&shard, encoded_trusted_getter)
		.map_err(|e| format!("{:?}", e))?;

	Ok(getter_result)
}

fn forward_dcap_quote_inner(params: Params) -> Result<OpaqueExtrinsic, String> {
	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() != 1 {
		return Err(format!(
			"Wrong number of arguments for IAS attestation report forwarding: {}, expected: {}",
			hex_encoded_params.len(),
			1
		))
	}

	let encoded_quote_to_forward: Vec<u8> =
		itp_utils::hex::decode_hex(&hex_encoded_params[0]).map_err(|e| format!("{:?}", e))?;

	let url = String::new();
	let ext = generate_dcap_ra_extrinsic_from_quote_internal(url, &encoded_quote_to_forward)
		.map_err(|e| format!("{:?}", e))?;

	let validator_access = get_validator_accessor_from_solo_or_parachain().unwrap();
	validator_access
		.execute_mut_on_validator(|v| v.send_extrinsics(vec![ext.clone()]))
		.unwrap();

	Ok(ext)
}

fn attesteer_forward_ias_attestation_report_inner(
	params: Params,
) -> Result<OpaqueExtrinsic, String> {
	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() != 1 {
		return Err(format!(
			"Wrong number of arguments for IAS attestation report forwarding: {}, expected: {}",
			hex_encoded_params.len(),
			1
		))
	}

	let ias_attestation_report =
		itp_utils::hex::decode_hex(&hex_encoded_params[0]).map_err(|e| format!("{:?}", e))?;

	let url = String::new();
	let ext = generate_ias_ra_extrinsic_from_der_cert_internal(url, &ias_attestation_report)
		.map_err(|e| format!("{:?}", e))?;

	let validator_access = get_validator_accessor_from_solo_or_parachain().unwrap();
	validator_access
		.execute_mut_on_validator(|v| v.send_extrinsics(vec![ext.clone()]))
		.unwrap();

	Ok(ext)
}
// FIXME: have the user submit a `ProofOfAttendance`
fn fetch_reputation_inner(params: Params) -> Result<Vec<Reputation>, String> {
	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() != 4 {
		return Err(format!(
			"Wrong number of arguments for IAS attestation report forwarding: {}, expected: {}",
			hex_encoded_params.len(),
			1
		))
	}

	let cid = itp_utils::hex::decode_hex(&hex_encoded_params[0]).map_err(|e| format!("{:?}", e))?;
	let cid = str::from_utf8(&cid).expect("cid should be a valid str value");
	let cid: CommunityIdentifier =
		CommunityIdentifier::from_str(cid).map_err(|e| format!("{:?}", e))?;

	let cindex =
		itp_utils::hex::decode_hex(&hex_encoded_params[1]).map_err(|e| format!("{:?}", e))?;
	let cindex = str::from_utf8(&cindex).expect("cindex should be a valid str value");
	let cindex = (cindex).parse::<u32>().expect("cid should be a valid integer value");

	let account =
		itp_utils::hex::decode_hex(&hex_encoded_params[2]).map_err(|e| format!("{:?}", e))?;
	if account.len() != 32 {
		return Err(format!("AccountId size is incorrect: {}, expected: {}", account.len(), 32))
	}
	let account: &[u8; 32] = account
		.as_slice()
		.try_into()
		.expect("Account vector size does not match the expected slice size.");

	let account = AccountId::from(*account);

	let number_of_reputations =
		itp_utils::hex::decode_hex(&hex_encoded_params[3]).map_err(|e| format!("{:?}", e))?;
	let number_of_reputations = str::from_utf8(&number_of_reputations)
		.expect("number_of_reputations should be a valid str value");
	let number_of_reputations = (number_of_reputations)
		.parse::<u32>()
		.expect("number_of_reputations should be a valid integer value");

	Ok(fetch_reputation(cid, cindex, account, number_of_reputations))
}

fn issue_nostr_badge_inner(params: Params) -> Result<(), String> {
	// Check reputation first - will be change later to have the user submit their `ProofOfAttendance`

	let reputations = fetch_reputation_inner(params.clone())?;
	let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();

	if verified_reputations == 0 {
		return Err("Failed to check reputations".to_string())
	}

	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() != 5 {
		return Err(format!(
			"Wrong number of arguments for Nostr badge request: {}, expected: {}",
			hex_encoded_params.len(),
			5
		))
	}
	let cid = itp_utils::hex::decode_hex(&hex_encoded_params[0]).map_err(|e| format!("{:?}", e))?;
	let cid = str::from_utf8(&cid).expect("cid should be a valid str value");
	let cid: CommunityIdentifier =
		CommunityIdentifier::from_str(cid).map_err(|e| format!("{:?}", e))?;

	let cindex =
		itp_utils::hex::decode_hex(&hex_encoded_params[1]).map_err(|e| format!("{:?}", e))?;
	let cindex = str::from_utf8(&cindex).expect("cindex should be a valid str value");
	let cindex = (cindex).parse::<u32>().expect("cid should be a valid integer value");

	let account =
		itp_utils::hex::decode_hex(&hex_encoded_params[2]).map_err(|e| format!("{:?}", e))?;
	if account.len() != 32 {
		return Err(format!("AccountId size is incorrect: {}, expected: {}", account.len(), 32))
	}
	let account: &[u8; 32] = account
		.as_slice()
		.try_into()
		.expect("Account vector size does not match the expected slice size.");

	let account = AccountId::from(*account);

	let nostr_pub_key =
		itp_utils::hex::decode_hex(&hex_encoded_params[3]).map_err(|e| format!("{:?}", e))?;
	let nostr_pub_key =
		str::from_utf8(&nostr_pub_key).expect("nostr_pub_key should be a valid str value");

	let nostr_pub_key = XOnlyPublicKey::from_bech32(nostr_pub_key)
		.expect("Nostr public key is not in bech32 format");

	let nostr_relay_url =
		itp_utils::hex::decode_hex(&hex_encoded_params[4]).map_err(|e| format!("{:?}", e))?;
	let nostr_relay_url =
		str::from_utf8(&nostr_relay_url).expect("nostr_relay_url should be a valid str value");

	let badge_def = create_nostr_badge_definition();
	let award = create_nostr_badge_award(badge_def.clone(), nostr_pub_key);

	let badge_def = badge_def.into_event();
	let award = award.into_event();

	send_nostr_events(vec![badge_def, award], nostr_relay_url);

	let _temp_tuple = (cid, cindex, account);

	Ok(())
}

fn create_nostr_badge_award(
	badge_definition: BadgeDefinition,
	awarded_pub_key: XOnlyPublicKey,
) -> BadgeAward {
	let badge_definition_event = badge_definition.into_event();
	let awarded_keys = vec![Tag::PubKey(awarded_pub_key, None)];

	let secp = Secp256k1::new();
	let keys = Keys::generate_with_secp(&secp);
	let ts = get_ts();

	nip58::BadgeAward::new(&badge_definition_event, awarded_keys, &keys, ts, &secp).unwrap()
}

fn create_nostr_badge_definition() -> BadgeDefinition {
	// Just for demo purposes, should be reworked
	let builder = nip58::BadgeDefinitionBuilder::new("likely_person".to_owned());
	let thumb_size = ImageDimensions(181, 151);
	let thumbs = vec![
		(
			"https://parachains.info/images/parachains/1625163231_encointer_logo.png".to_owned(),
			Some(thumb_size),
		),
		(
			"https://parachains.info/images/parachains/1625163231_encointer_logo.png".to_owned(),
			None,
		),
	];
	let builder = builder
		.image("https://parachains.info/images/parachains/1625163231_encointer_logo.png".to_owned())
		.thumbs(thumbs)
		.image_dimensions(ImageDimensions(181, 151));

	let secp = Secp256k1::new();
	//let keys = Keys::generate_with_secp(&secp);\
	let keys =
		Keys::from_sk_str("nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r", &secp)
			.unwrap();
	let ts = get_ts();

	builder.build(&keys, ts, &secp).unwrap()
}

pub fn sidechain_io_handler<ImportFn, Error>(import_fn: ImportFn) -> IoHandler
where
	ImportFn: Fn(SignedBlock) -> Result<(), Error> + Sync + Send + 'static,
	Error: std::fmt::Debug,
{
	let io = IoHandler::new();
	import_block_api::add_import_block_rpc_method(import_fn, io)
}

#[cfg(feature = "test")]
pub mod tests {
	use super::*;
	use std::string::ToString;

	pub fn test_given_io_handler_methods_then_retrieve_all_names_as_string() {
		let mut io = IoHandler::new();
		let method_names: [&str; 4] = ["method1", "another_method", "fancy_thing", "solve_all"];

		for method_name in method_names.iter() {
			io.add_sync_method(method_name, |_: Params| Ok(Value::String("".to_string())));
		}

		let method_string = get_all_rpc_methods_string(&io);

		for method_name in method_names.iter() {
			assert!(method_string.contains(method_name));
		}
	}
}
