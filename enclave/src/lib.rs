/*
	Copyright 2019 Supercomputing Systems AG

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

#![crate_name = "substratee_worker_enclave"]
#![crate_type = "staticlib"]

#![cfg_attr(not(target_env = "sgx"), no_std)]
#![cfg_attr(target_env = "sgx", feature(rustc_private))]

extern crate base64;
extern crate bit_vec;
extern crate blake2_no_std;
extern crate chrono;
extern crate crypto;
extern crate env_logger;
extern crate httparse;
extern crate itertools;
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate my_node_runtime;
extern crate num_bigint;
extern crate parity_codec;
extern crate primitive_types;
extern crate primitives;
extern crate runtime_primitives;
extern crate rust_base58;
extern crate rustls;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate sgx_crypto_helper;
extern crate sgx_rand;
extern crate sgx_serialize;
#[macro_use]
extern crate sgx_serialize_derive;
extern crate sgx_tcrypto;
extern crate sgx_trts;
extern crate sgx_tse;
extern crate sgx_tseal;
#[cfg(not(target_env = "sgx"))]
#[macro_use]
extern crate sgx_tstd as std;
extern crate sgx_types;
extern crate sgxwasm;
extern crate untrusted;
extern crate wasmi;
extern crate webpki;
extern crate webpki_roots;
extern crate yasna;


use crypto::ed25519::{keypair, signature};
use my_node_runtime::{Call, Hash, SubstraTEEProxyCall, UncheckedExtrinsic};
use parity_codec::{Compact, Decode, Encode};
use primitive_types::U256;
use primitives::ed25519;
use runtime_primitives::generic::Era;
use rust_base58::ToBase58;
use sgx_crypto_helper::rsa3072::Rsa3072KeyPair;
use sgx_crypto_helper::RsaKeyPair;
use sgx_serialize::{DeSerializeHelper, SerializeHelper};
use sgx_types::{sgx_sha256_hash_t, sgx_status_t};

use constants::{COUNTERSTATE, ED25519_SEALED_KEY_FILE, RSA3072_SEALED_KEY_FILE};
use std::collections::HashMap;
use std::sgxfs::SgxFile;
use std::slice;
use std::string::String;
use std::string::ToString;
use std::vec::Vec;

mod constants;
mod utils;
mod wasm;
mod attestation;

pub mod cert;
pub mod hex;

pub const CERTEXPIRYDAYS: i64 = 90i64;

#[no_mangle]
pub unsafe extern "C" fn get_rsa_encryption_pubkey(pubkey: *mut u8, pubkey_size: u32) -> sgx_status_t {

	// initialize the logging environment in the enclave
	env_logger::init();

	if let Err(x) = SgxFile::open(RSA3072_SEALED_KEY_FILE) {
		info!("[Enclave] Keyfile not found, creating new! {}", x);
		if let Err(status) = create_sealed_rsa3072_keypair() {
			return status
		}
	}

	let rsa_pubkey = match utils::read_rsa_pubkey() {
		Ok(key) => key,
		Err(status) => return status,
	};

	let rsa_pubkey_json = match serde_json::to_string(&rsa_pubkey) {
		Ok(k) => k,
		Err(x) => {
			println!("[Enclave] can't serialize rsa_pubkey {:?} {}", rsa_pubkey, x);
			return sgx_status_t::SGX_ERROR_UNEXPECTED;
		}
	};

	let pubkey_slice = slice::from_raw_parts_mut(pubkey, pubkey_size as usize);

	// split the pubkey_slice at the length of the rsa_pubkey_json
	// and fill the right side with whitespace so that the json can be decoded later on
	let (left, right) = pubkey_slice.split_at_mut(rsa_pubkey_json.len());
	left.clone_from_slice(rsa_pubkey_json.as_bytes());
	right.iter_mut().for_each(|x| *x = 0x20);

	sgx_status_t::SGX_SUCCESS
}

fn create_sealed_rsa3072_keypair() -> Result<sgx_status_t, sgx_status_t> {
	let rsa_keypair = Rsa3072KeyPair::new().unwrap();
	let rsa_key_json = serde_json::to_string(&rsa_keypair).unwrap();
	// println!("[Enclave] generated RSA3072 key pair. Cleartext: {}", rsa_key_json);
	utils::write_file(rsa_key_json.as_bytes(), RSA3072_SEALED_KEY_FILE)
}

#[no_mangle]
pub unsafe extern "C" fn get_ecc_signing_pubkey(pubkey: * mut u8, pubkey_size: u32) -> sgx_status_t {

	// initialize the logging environment in the enclave
	env_logger::init();

	match SgxFile::open(ED25519_SEALED_KEY_FILE) {
		Ok(_k) => (),
		Err(x) => {
			info!("[Enclave] Keyfile not found, creating new! {}", x);
			if let Err(status) = utils::create_sealed_ed25519_seed() {
				return status;
			}
		},
	}

	let _seed = match utils::get_ecc_seed() {
		Ok(seed) => seed,
		Err(status) => return status,
	};

	let (_privkey, _pubkey) = keypair(&_seed);
	info!("[Enclave] Restored ECC pubkey: {:?}", _pubkey.to_base58());

	let pubkey_slice = slice::from_raw_parts_mut(pubkey, pubkey_size as usize);
	pubkey_slice.clone_from_slice(&_pubkey);

	sgx_status_t::SGX_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn call_counter_wasm(
	req_bin: *const u8,
	req_length: usize,
	ciphertext: *mut u8,
	ciphertext_size: u32,
	hash: *const u8,
	hash_size: u32,
	nonce: *const u8,
	nonce_size: u32,
	wasm_hash: *const u8,
	wasm_hash_size: u32,
	unchecked_extrinsic: *mut u8,
	unchecked_extrinsic_size: u32
) -> sgx_status_t {


	let ciphertext_slice = slice::from_raw_parts(ciphertext, ciphertext_size as usize);
	let hash_slice       = slice::from_raw_parts(hash, hash_size as usize);
	let mut nonce_slice  = slice::from_raw_parts(nonce, nonce_size as usize);
	let extrinsic_slice  = slice::from_raw_parts_mut(unchecked_extrinsic, unchecked_extrinsic_size as usize);

	debug!("[Enclave] Read RSA keypair");
	let rsa_keypair = match utils::read_rsa_keypair() {
		Ok(pair) => pair,
		Err(status) => return status,
	};

	debug!("[Enclave] Read RSA keypair done");

	// decode the payload
	println!("    [Enclave] Decode the payload");
	let plaintext_vec = utils::decode_payload(&ciphertext_slice, &rsa_keypair);
	let plaintext_string = String::from_utf8(plaintext_vec.clone()).unwrap();
	let msg: Message = serde_json::from_str(&plaintext_string).unwrap();

	println!("    [Enclave] Message decoded:");
	println!("    [Enclave]   account   = {}", msg.account);
	println!("    [Enclave]   increment = {}", msg.amount);
	println!("    [Enclave]   sha256    = {:?}", msg.sha256);

	// get the calculated SHA256 hash
	let wasm_hash_slice = slice::from_raw_parts(wasm_hash, wasm_hash_size as usize);
	let wasm_hash_calculated: sgx_sha256_hash_t = serde_json::from_slice(wasm_hash_slice).unwrap();

	if let Err(status) = wasm::compare_hashes(wasm_hash_calculated, msg.sha256) {
		return status;
	}

	let state = match utils::read_counterstate(COUNTERSTATE) {
		Ok(state) => state,
		Err(status) => return status,
	};

	let helper = DeSerializeHelper::<AllCounts>::new(state);
	let mut counter = helper.decode().unwrap();

	// get the current counter value of the account or initialize with 0
	let counter_value_old: u32 = *counter.entries.entry(msg.account.to_string()).or_insert(0);
	info!("    [Enclave] Current counter state of '{}' = {}", msg.account, counter_value_old);

	println!("    [Enclave] Executing WASM code");
	let req_slice = slice::from_raw_parts(req_bin, req_length);
	let action_req: sgxwasm::SgxWasmAction = serde_json::from_slice(req_slice).unwrap();

	if let Err(status) = wasm::invoke_wasm_action(action_req, msg, &mut counter) {
		return status;
	}


	// get information for composing the extrinsic
	let _seed = match utils::get_ecc_seed() {
		Ok(seed) => seed,
		Err(status) => return status,
	};
	let nonce = U256::decode(&mut nonce_slice).unwrap();
	let genesis_hash = utils::hash_from_slice(hash_slice);
	let call_hash = utils::blake2s(&plaintext_vec);
	debug!("[Enclave]: Call hash {:?}", call_hash);

	let ex = compose_extrinsic(_seed, &call_hash, nonce, genesis_hash);
	let encoded = ex.encode();

	// split the extrinsic_slice at the length of the encoded extrinsic
	// and fill the right side with whitespace
	let (left, right) = extrinsic_slice.split_at_mut(encoded.len());
	left.clone_from_slice(&encoded);
	right.iter_mut().for_each(|x| *x = 0x20);

	// write the counter state
	if let Err(status) = write_counter_state(counter) {
		return status;
	}

	sgx_status_t::SGX_SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn get_counter(account: *const u8, account_size: u32, value: *mut u32) -> sgx_status_t {
	let account_slice = slice::from_raw_parts(account, account_size as usize);
	let acc_str = std::str::from_utf8(account_slice).unwrap();

	let state_vec = match utils::read_counterstate(COUNTERSTATE) {
		Ok(state) => state,
		Err(status) => return status,
	};

	let helper = DeSerializeHelper::<AllCounts>::new(state_vec);
	let mut counter = helper.decode().unwrap();
	let ref_mut = &mut *value;
	*ref_mut = *counter.entries.entry(acc_str.to_string()).or_insert(0);
	sgx_status_t::SGX_SUCCESS
}

fn write_counter_state(value: AllCounts) -> Result<sgx_status_t, sgx_status_t> {
	let helper = SerializeHelper::new();
	let c = helper.encode(value).unwrap();
	utils::write_file(&c, COUNTERSTATE)
}

#[derive(Serializable, DeSerializable, Debug)]
pub struct AllCounts {
	entries: HashMap<String, u32>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
	account: String,
	amount: u32,
	sha256: sgx_sha256_hash_t
}

pub fn compose_extrinsic(seed: Vec<u8>, call_hash: &[u8], nonce: U256, genesis_hash: Hash) -> UncheckedExtrinsic {
	let (_privkey, _pubkey) = keypair(&seed);

	let era = Era::immortal();
	let function = Call::SubstraTEEProxy(SubstraTEEProxyCall::confirm_call(call_hash.to_vec()));

	let index = nonce.low_u64();
	let raw_payload = (Compact(index), function, era, genesis_hash);

	let sign = raw_payload.using_encoded(|payload| if payload.len() > 256 {
		// should not be thrown as we calculate a 32 byte hash ourselves
		error!("unsupported payload size");
		signature(&[0u8; 64], &_privkey)
	} else {
		//println!("signing {}", HexDisplay::from(&payload));
		signature(payload, &_privkey)
	});

	let signerpub = ed25519::Public::from_raw(_pubkey);
	let signature = ed25519::Signature::from_raw(sign);

	UncheckedExtrinsic::new_signed(
		index,
		raw_payload.1,
		signerpub.into(),
		signature,
		era,
	)
}

