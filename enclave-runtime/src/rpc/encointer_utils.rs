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
use crate::{initialization::global_components::GLOBAL_OCALL_API_COMPONENT, Vec};
use codec::Decode;
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use itp_component_container::ComponentGetter;
use itp_ocall_api::EnclaveOnChainOCallApi;
use itp_stf_primitives::types::AccountId;
use itp_storage::{storage_double_map_key, StorageHasher};
use itp_types::{WorkerRequest, WorkerResponse};
use log::error;

pub fn fetch_reputation(
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	account: AccountId,
	number_of_reputations: CeremonyIndexType,
) -> Vec<Reputation> {
	if cindex < number_of_reputations {
		error!(
			"current ceremony index is {}, can't fetch last {} ceremonies.",
			cindex, number_of_reputations
		);
		return vec![]
	}
	query_last_n_reputations(&account, cid, cindex, number_of_reputations)
}

fn query_last_n_reputations(
	prover: &AccountId,
	cid: CommunityIdentifier,
	current_cindex: CeremonyIndexType,
	n: CeremonyIndexType,
) -> Vec<Reputation> {
	(1..=n)
		.map(|i| get_reputation_ocall_api(prover, cid, current_cindex - i))
		.collect()
}

fn get_reputation_ocall_api(
	prover: &AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) -> Reputation {
	println!("cid is :{}, cindex is: {}", cid, cindex.clone());
    let unverified_reputation = Reputation::Unverified;

	let ocall_api = GLOBAL_OCALL_API_COMPONENT.get();
    if let Err(e) = ocall_api {
        error!("failed to get OCALL API");
            return unverified_reputation;
    }
    let ocall_api = ocall_api.expect("Failed to get OCALL API, but it should have succeded.");
	let storage_hash = storage_double_map_key(
		"EncointerCeremonies",
		"ParticipantReputation",
		&(cid, cindex),
		&StorageHasher::Blake2_128Concat,
		prover,
		&StorageHasher::Blake2_128Concat,
	);
	println!("storage_hash is :{:#?}", &storage_hash);

	let requests = vec![WorkerRequest::ChainStorage(storage_hash, None)];
	let mut resp: Vec<WorkerResponse<Vec<u8>>> = match ocall_api.worker_request(requests) {
		Ok(response) => response,
		Err(e) => {
            error!("Worker response decode failed. Error: {:?}", e);
            return unverified_reputation;
        }
	};

	let first = resp.pop().map_err(
        |e| {
            error!("Worker should have responded, error: {:#?}", e);
            return unverified_reputation;
        },);
	println!("Worker response: {:?}", first);

	let (_key, value, _proof) = match first {
		WorkerResponse::ChainStorage(storage_key, value, proof) => (storage_key, value, proof),
	};

	match value {
		None => Reputation::Unverified,
		Some(v) => {
			let reputation: Reputation =
				Decode::decode(&mut v.as_slice()).expect("Failed to decode value after fetching");
			reputation
		},
	}
}
