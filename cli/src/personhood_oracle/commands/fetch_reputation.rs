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
use itp_storage::{StorageHasher, StorageProof, StorageProofChecker};
use itp_types::H256;
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

pub type ReputationReadProof = substrate_api_client::api::error::Result<Option<ReadProof<H256>>>;
pub type ReputationsWithReadProofs = (Vec<Reputation>, Vec<ReputationReadProof>);

impl FetchReputationCmd {
	pub fn run(&self, cli: &Cli) {
		let api = get_chain_api(&cli);
		let cid = CommunityIdentifier::from_str(&self.cid).unwrap();
		let cindex = get_ceremony_index(&api);
		let account = get_accountid_from_str(&self.account);

		if let Some((reputations, read_proofs)) = FetchReputationCmd::fetch_reputation(
			&api,
			cid,
			cindex,
			account.clone(),
			self.number_of_reputations,
		) {
			let verified_reputations = reputations.iter().filter(|rep| rep.is_verified()).count();
			println!("reputation for {} is: {:#?}", account, reputations);
			println!(
				"verified reputatations number: {} out of:{}",
				verified_reputations,
				reputations.len()
			);
			println!("read proof is: {:#?}", read_proofs);
		}
	}

	// FIXME: change to result once it is an RPC method
	pub fn fetch_reputation(
		api: &ParentchainApi,
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		account: AccountId,
		number_of_reputations: CeremonyIndexType,
	) -> Option<ReputationsWithReadProofs> {
		if cindex < number_of_reputations {
			error!(
				"current ceremony index is {}, can't fetch last {} ceremonies.",
				cindex, number_of_reputations
			);
			return None
		}

		// TODO fetch the storage item instead, to have builtin readproof validation.
		let reputations =
			query_last_n_reputations(&api, &account, cid, cindex, number_of_reputations);

		let read_proofs = get_read_proofs(&api, &account, cid, cindex, number_of_reputations);
		// TODO add validation here as a new function
		//validate_reputations(read_proofs.clone(), cid, cindex, account);
		Some((reputations, read_proofs))
	}

	pub fn validate_reputations(
		proofs: &Vec<StorageProof>,
		blocks_merkle_roots: &Vec<sp_core::H256>,
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		prover: &AccountId,
	) {
		if proofs.len() != blocks_merkle_roots.len() {
			//return Err(Error::ParentChainSync)
			panic!("length mismatch");
		}

		let reputations_key = itp_storage::storage_double_map_key(
			"EncointerCeremonies",
			"ParticipantReputation",
			&(cid, cindex),
			&StorageHasher::Blake2_128Concat,
			&prover,
			&StorageHasher::Blake2_128Concat,
		);

		for (proof, root) in proofs.iter().zip(blocks_merkle_roots.iter()) {
			StorageProofChecker::<BlakeTwo256>::check_proof(
				*root,
				reputations_key.as_slice(),
				proof.clone(),
			)
			.unwrap()
			.unwrap();
		}
	}
}

fn get_reputation(
	api: &ParentchainApi,
	prover: &AccountId,
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
) -> Reputation {
	println!("cid is :{}, cindex is: {}", cid.clone(), cindex.clone());
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

pub(crate) fn get_ceremony_index(api: &ParentchainApi) -> CeremonyIndexType {
	api.get_storage_value("EncointerScheduler", "CurrentCeremonyIndex", None)
		.unwrap()
		.unwrap()
}

fn query_last_n_reputations(
	api: &ParentchainApi,
	prover: &AccountId,
	cid: CommunityIdentifier,
	current_cindex: CeremonyIndexType,
	n: CeremonyIndexType,
) -> Vec<Reputation> {
	(1..=n)
		.map(|i| get_reputation(api, prover, cid.clone(), current_cindex - i))
		.collect()
}

fn get_read_proofs(
	api: &ParentchainApi,
	prover: &AccountId,
	cid: CommunityIdentifier,
	current_cindex: CeremonyIndexType,
	n: CeremonyIndexType,
) -> Vec<substrate_api_client::api::error::Result<Option<ReadProof<H256>>>> {
	(1..=n)
		.map(|i| {
			api.get_storage_double_map_proof(
				"EncointerCeremonies",
				"ParticipantReputation",
				(cid.clone(), current_cindex - i),
				prover.clone(),
				None,
			)
		})
		.collect()
}
