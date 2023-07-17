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
use crate::Vec;
use encointer_primitives::{
	ceremonies::Reputation, communities::CommunityIdentifier, scheduler::CeremonyIndexType,
};
use ita_stf::helpers::get_storage_double_map;
use itp_stf_primitives::types::AccountId;
use itp_storage::StorageHasher;
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

	// TODO fetch the storage item instead, to have builtin readproof validation.
	//let reputations = query_last_n_reputations(&account, cid, cindex, number_of_reputations);

	//let read_proofs = get_read_proofs(&account, cid, cindex, number_of_reputations);
	// TODO add validation here as a new function
	//validate_reputations(read_proofs.clone(), cid, cindex, account);
	//reputations
	query_last_n_reputations(&account, cid, cindex, number_of_reputations)
}

fn query_last_n_reputations(
	prover: &AccountId,
	cid: CommunityIdentifier,
	current_cindex: CeremonyIndexType,
	n: CeremonyIndexType,
) -> Vec<Reputation> {
	(1..=n).map(|i| get_reputation(prover, cid, current_cindex - i)).collect()
}

fn get_reputation(
	prover: &AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) -> Reputation {
	println!("cid is :{}, cindex is: {}", cid, cindex.clone());
	let reputation = get_storage_double_map(
		"EncointerCeremonies",
		"ParticipantReputation",
		&(cid, cindex),
		&StorageHasher::Blake2_128Concat,
		&prover,
		&StorageHasher::Blake2_128Concat,
	);
	reputation.unwrap_or(Reputation::Unverified)
}
