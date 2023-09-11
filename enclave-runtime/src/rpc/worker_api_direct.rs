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
	utils::{
		get_extrinsic_factory_from_target_b_solo_or_parachain,
		get_validator_accessor_from_solo_or_parachain,
		get_validator_accessor_from_target_b_solo_or_parachain,
	},
};
use codec::{Decode, Encode};
use core::result::Result;
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use ita_sgx_runtime::Runtime;
use itc_parentchain::light_client::{concurrent_access::ValidatorAccess, ExtrinsicSender};
use itp_extrinsics_factory::CreateExtrinsics;
use itp_primitives_cache::{GetPrimitives, GLOBAL_PRIMITIVES_CACHE};
use itp_rpc::RpcReturnValue;
use itp_sgx_crypto::key_repository::AccessPubkey;
use itp_stf_executor::getter_executor::ExecuteGetter;
use itp_stf_primitives::types::AccountId;
use itp_top_pool_author::traits::AuthorApi;
use itp_types::{DirectRequestStatus, OpaqueCall, Request, ShardIdentifier, H256};
use itp_utils::{FromHexPrefixed, ToHexPrefixed};
use its_primitives::types::block::SignedBlock;
use its_sidechain::rpc_handler::{direct_top_pool_api, import_block_api};
use jsonrpc_core::{serde_json::json, IoHandler, Params, Value};
use log::*;
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
			Ok(id) => RpcReturnValue {
				do_watch: false,
				value: id.as_bytes().to_vec(),
				status: DirectRequestStatus::Ok,
			}
			.to_hex(),
			Err(error) => compute_hex_encoded_return_error(error.as_str()),
		};

		Ok(json!(json_value))
	});

	// personhoodoracle_issueNodeTemplateXt
	let personhoodoracle_issue_node_template_xt: &str = "personhoodoracle_issueNodeTemplateXt";
	io.add_sync_method(personhoodoracle_issue_node_template_xt, move |params: Params| {
		let json_value = match issue_node_template_xt_inner(params) {
			Ok(id) => RpcReturnValue {
				do_watch: false,
				value: "Ok".as_bytes().to_vec(),
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

fn personhoodoracle_parse_params(
	params: Params,
) -> Result<(CommunityIdentifier, CeremonyIndexType, AccountId), String> {
	let hex_encoded_params = params.parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() < 4 {
		return Err(format!(
			"Wrong number of arguments for fetch_reputation_inner: {}, expected: {}",
			hex_encoded_params.len(),
			1
		))
	}

	let cid = itp_utils::hex::decode_hex(&hex_encoded_params[0]).map_err(|e| format!("{:?}", e))?;
	let cid: CommunityIdentifier =
		Decode::decode(&mut cid.as_slice()).map_err(|e| format!("{:?}", e))?;

	let cindex =
		itp_utils::hex::decode_hex(&hex_encoded_params[1]).map_err(|e| format!("{:?}", e))?;
	let cindex: CeremonyIndexType =
		Decode::decode(&mut cindex.as_slice()).map_err(|e| format!("{:?}", e))?;

	let account =
		itp_utils::hex::decode_hex(&hex_encoded_params[2]).map_err(|e| format!("{:?}", e))?;
	let account: AccountId =
		Decode::decode(&mut account.as_slice()).map_err(|e| format!("{:?}", e))?;

	Ok((cid, cindex, account))
}
// FIXME: have the user submit a `ProofOfAttendance`
fn fetch_reputation_inner(params: Params) -> Result<Vec<Reputation>, String> {
	let (cid, cindex, account) = personhoodoracle_parse_params(params)?;
	trace!("reputation for account {:?} for community {} at cycle {}", account, cid, cindex);
	Ok(fetch_reputation(cid, cindex, account))
}

fn issue_nostr_badge_inner(params: Params) -> Result<nostr::EventId, String> {
	trace!("evaluating reputation to maybe issue a nostr badge");
	// Check reputation first - will be change later to have the user submit their `ProofOfAttendance`

	let reputations = fetch_reputation_inner(params.clone())?;
	let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();

	if verified_reputations == 0 {
		return Err("The user does not have any reputation".to_string())
	}

	let hex_encoded_params =
		params.clone().parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() < 5 {
		return Err(format!(
			"Wrong number of arguments for Nostr badge request: {}, expected: {}",
			hex_encoded_params.len(),
			6
		))
	}
	let (cid, cindex, account) = personhoodoracle_parse_params(params)?;

	let nostr_pub_key =
		itp_utils::hex::decode_hex(&hex_encoded_params[3]).map_err(|e| format!("{:?}", e))?;
	let nostr_pub_key_str: String =
		Decode::decode(&mut nostr_pub_key.as_slice()).map_err(|e| format!("{:?}", e))?;
	let nostr_pub_key =
		XOnlyPublicKey::from_bech32(&nostr_pub_key_str).map_err(|e| format!("{:?}", e))?;

	let nostr_relay_url =
		itp_utils::hex::decode_hex(&hex_encoded_params[4]).map_err(|e| format!("{:?}", e))?;
	let nostr_relay_url: String =
		Decode::decode(&mut nostr_relay_url.as_slice()).map_err(|e| format!("{:?}", e))?;

	// todo: derive from enclave signing key
	let nostr_issuers_private_key =
		"nsec1punh8ys9ewzjhuuzh6dtna2d72l8d2pc2cyuuh93g9h73uedg5nq9z4p7l".to_string();

	let secp = Secp256k1::new();
	let signer_key =
		Keys::from_sk_str(&nostr_issuers_private_key, &secp).map_err(|e| format!("{:?}", e))?;

	let badge_def = create_nostr_badge_definition(&signer_key, verified_reputations as u32);
	println!("prepared nostr badge definition");
	debug!("  {:?}", badge_def);
	// todo: set NIP-40 expiry timestamp
	let award = create_nostr_badge_award(badge_def.clone(), nostr_pub_key, &signer_key);
	println!("prepared nostr badge award for {}", nostr_pub_key_str);
	debug!("  {:?}", award);
	let badge_def = badge_def.into_event();
	let award = award.into_event();
	println!("sending to nostr relay at {}", nostr_relay_url);
	let nostr_events = vec![badge_def, award.clone()];
	send_nostr_events(nostr_events, &nostr_relay_url)
		.map_err(|e| format!("Failed to send nostr events: {:?}", e))?;

	let _temp_tuple = (cid, cindex, account);

	Ok(award.id)
}

fn create_nostr_badge_award(
	badge_definition: BadgeDefinition,
	awarded_pub_key: XOnlyPublicKey,
	signer_key: &Keys,
) -> BadgeAward {
	let badge_definition_event = badge_definition.into_event();
	let awarded_keys = vec![Tag::PubKey(awarded_pub_key, None)];

	let secp = Secp256k1::new();
	let ts = get_ts();

	nip58::BadgeAward::new(&badge_definition_event, awarded_keys, signer_key, ts, &secp).unwrap()
}

fn create_nostr_badge_definition(signer_key: &Keys, reputation: u32) -> BadgeDefinition {
	// Just for demo purposes, should be reworked
	let builder =
		nip58::BadgeDefinitionBuilder::new(format!("personhood_{}", reputation).to_owned());
	let builder = builder
		.name(format!("Personhood Confidence {}/5 Verified by Encointer (TESTING)", reputation))
		.description(format!("This badge is only issued once every 10 days for reputables on the Encointer network who have attended proof of personhood cycles (see https://encointer.org). \
			Each person owning an account bearing this badge has verifiably attended (in person) {} of the last 5 cycles. \
			There can always ever be maximally as many 5-of-5 badges issued as there are human participants in encointer communities.\
			For enhanced privacy, this badge is unlinkable to the account used on Encointer Network (Unlinked by Integritee's trusted execution environment oracle SDK: https://integritee.network).\
			THE ISSUER OF THIS BADGE IS STILL RUNNING IN TEE TESTING MODE. DO NOT TRUST THIS BADGE JUST YET", reputation))
		.image("https://cdn.nostr.build/i/1e779aabd6fe190ca26f5211bab84a8f3642ed1d74f7cda70b73d96779b80f34.png".to_owned())
		.image_dimensions(ImageDimensions(1024, 1024))
		.thumbs(vec![
			(
				"https://cdn.nostr.build/i/aaf7ee0c2ae330fdb6dc48b6373ca19bb61b0bf11abc154358ecd679d17a5d1d.png".to_owned(),
				Some(ImageDimensions(512, 512)),
			),
			(
				"https://cdn.nostr.build/i/31b726aab0bd615e46c2f1b682208ab262a77e6618637b25872499d370c2c91f.png".to_owned(),
				Some(ImageDimensions(256, 256)),
			),
			(
				"https://cdn.nostr.build/i/da811af1838f530f0dfdcc2a419e207f36b6a0a4ee935631c43900add354026d.png".to_owned(),
				Some(ImageDimensions(64, 64)),
			),
		]);

	let secp = Secp256k1::new();
	let ts = get_ts();

	builder.build(signer_key, ts, &secp).unwrap()
}

fn issue_node_template_xt_inner(params: Params) -> Result<(), String> {
	trace!("evaluating reputation to maybe issue something on node template");
	// Check reputation first - will be change later to have the user submit their `ProofOfAttendance`

	let reputations = fetch_reputation_inner(params.clone())?;
	let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();

	if verified_reputations == 0 {
		return Err("The user does not have any reputation".to_string())
	}

	let hex_encoded_params =
		params.clone().parse::<Vec<String>>().map_err(|e| format!("{:?}", e))?;

	if hex_encoded_params.len() < 4 {
		return Err(format!(
			"Wrong number of arguments for Nostr badge request: {}, expected: {}",
			hex_encoded_params.len(),
			5
		))
	}
	let (cid, cindex, account) = personhoodoracle_parse_params(params)?;

	let subject_template_idx_bytes =
		itp_utils::hex::decode_hex(&hex_encoded_params[3]).map_err(|e| format!("{:?}", e))?;
	let subject_template_idx: u32 = Decode::decode(&mut subject_template_idx_bytes.as_slice())
		.map_err(|e| format!("{:?}", e))?;

	println!("subject index on template is {}", subject_template_idx);

	let call = OpaqueCall::from_tuple(&((7u8, 0u8), subject_template_idx));
	println!("encoded call: 0x{}", hex::encode(call.encode()));
	let extrinsics_factory = get_extrinsic_factory_from_target_b_solo_or_parachain().unwrap();
	let extrinsics: Vec<OpaqueExtrinsic> =
		extrinsics_factory.create_extrinsics(&[call], None).unwrap();
	println!("sending {} extrinsics: {:?}", extrinsics.len(), extrinsics);
	let validator_access = get_validator_accessor_from_target_b_solo_or_parachain().unwrap();
	validator_access
		.execute_mut_on_validator(|v| v.send_extrinsics(extrinsics))
		.unwrap();

	Ok(())
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
