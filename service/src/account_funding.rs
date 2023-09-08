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

use crate::error::{Error, ServiceResult};
use itp_node_api::api_client::{AccountApi, ParentchainApi, ParentchainExtrinsicSigner};
use itp_settings::worker::{
	EXISTENTIAL_DEPOSIT_FACTOR_FOR_INIT_FUNDS, REGISTERING_FEE_FACTOR_FOR_INIT_FUNDS,
};
use itp_types::parentchain::Balance;
use log::*;
use sp_core::{
	crypto::{AccountId32, Ss58Codec},
	Pair,
};
use sp_keyring::AccountKeyring;
use sp_runtime::MultiAddress;
use substrate_api_client::{
	extrinsic::BalancesExtrinsics, GetBalance, GetTransactionPayment, SubmitAndWatchUntilSuccess,
};

/// Information about the enclave on-chain account.
pub trait EnclaveAccountInfo {
	fn free_balance(&self) -> ServiceResult<Balance>;
}

pub struct EnclaveAccountInfoProvider {
	node_api: ParentchainApi,
	account_id: AccountId32,
}

impl EnclaveAccountInfo for EnclaveAccountInfoProvider {
	fn free_balance(&self) -> ServiceResult<Balance> {
		self.node_api.get_free_balance(&self.account_id).map_err(|e| e.into())
	}
}

impl EnclaveAccountInfoProvider {
	pub fn new(node_api: ParentchainApi, account_id: AccountId32) -> Self {
		EnclaveAccountInfoProvider { node_api, account_id }
	}
}

pub fn setup_account_funding(
	api: &ParentchainApi,
	accountid: &AccountId32,
	encoded_extrinsic: Vec<u8>,
	is_development_mode: bool,
) -> ServiceResult<()> {
	// Account funds
	if is_development_mode {
		// Development mode, the faucet will ensure that the enclave has enough funds
		ensure_account_has_funds(api, accountid)?;
	} else {
		// Production mode, there is no faucet.
		let registration_fees = enclave_registration_fees(api, encoded_extrinsic)?;
		info!("Registration fees = {:?}", registration_fees);
		let free_balance = api.get_free_balance(accountid)?;
		info!("TEE's free balance = {:?}", free_balance);

		let min_required_funds =
			registration_fees.saturating_mul(REGISTERING_FEE_FACTOR_FOR_INIT_FUNDS);
		let missing_funds = min_required_funds.saturating_sub(free_balance);

		if missing_funds > 0 {
			// If there are not enough funds, then the user can send the missing TEER to the enclave address and start again.
			println!(
				"Enclave account: {:}, missing funds {}",
				accountid.to_ss58check(),
				missing_funds
			);
			return Err(Error::Custom(
				"Enclave does not have enough funds on the parentchain to register.".into(),
			))
		}
	}
	Ok(())
}

// Alice plays the faucet and sends some funds to the account if balance is low
fn ensure_account_has_funds(api: &ParentchainApi, accountid: &AccountId32) -> Result<(), Error> {
	info!("Ensuring funds for Account: {} on chain with genesis hash {}", accountid, api.genesis_hash());
	// check account balance
	let free_balance = api.get_free_balance(accountid)?;
	info!("TEE's free balance = {:?} (Account: {})", free_balance, accountid);

	let existential_deposit = api.get_existential_deposit()?;
	info!("Existential deposit is = {:?}", existential_deposit);

	let min_required_funds =
		existential_deposit.saturating_mul(EXISTENTIAL_DEPOSIT_FACTOR_FOR_INIT_FUNDS);
	let missing_funds = min_required_funds.saturating_sub(free_balance);

	if missing_funds > 0 {
		info!("Transfer {:?} from Alice to {}", missing_funds, accountid);
		bootstrap_funds_from_alice(api, accountid, missing_funds)?;
	}
	Ok(())
}

fn enclave_registration_fees(
	api: &ParentchainApi,
	encoded_extrinsic: Vec<u8>,
) -> Result<u128, Error> {
	let reg_fee_details = api.get_fee_details(encoded_extrinsic.into(), None)?;
	match reg_fee_details {
		Some(details) => match details.inclusion_fee {
			Some(fee) => Ok(fee.inclusion_fee()),
			None => Err(Error::Custom(
				"Inclusion fee for the registration of the enclave is None!".into(),
			)),
		},
		None =>
			Err(Error::Custom("Fee Details for the registration of the enclave is None !".into())),
	}
}

// Alice sends some funds to the account
fn bootstrap_funds_from_alice(
	api: &ParentchainApi,
	accountid: &AccountId32,
	funding_amount: u128,
) -> Result<(), Error> {
	let alice = AccountKeyring::Alice.pair();
	let alice_acc = AccountId32::from(*alice.public().as_array_ref());

	let alice_free = api.get_free_balance(&alice_acc)?;
	trace!("    Alice's free balance = {:?}", alice_free);
	let nonce = api.get_nonce_of(&alice_acc)?;
	trace!("    Alice's Account Nonce is {}", nonce);

	if funding_amount > alice_free {
		println!(
            "funding amount is too high: please change EXISTENTIAL_DEPOSIT_FACTOR_FOR_INIT_FUNDS ({:?})",
            funding_amount
        );
		return Err(Error::ApplicationSetup)
	}

	let mut alice_signer_api = api.clone();
	alice_signer_api.set_signer(ParentchainExtrinsicSigner::new(alice));

	println!("[+] send extrinsic: bootstrap funding Enclave from Alice's funds");
	let xt = alice_signer_api
		.balance_transfer_allow_death(MultiAddress::Id(accountid.clone()), funding_amount);
	let xt_report = alice_signer_api.submit_and_watch_extrinsic_until_success(xt, false)?;
	info!(
		"[<] L1 extrinsic success. extrinsic hash: {:?} / status: {:?}",
		xt_report.extrinsic_hash, xt_report.status
	);
	// Verify funds have arrived.
	let free_balance = alice_signer_api.get_free_balance(accountid);
	trace!("TEE's NEW free balance = {:?}", free_balance);

	Ok(())
}
