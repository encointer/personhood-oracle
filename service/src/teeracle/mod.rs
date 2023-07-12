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

use crate::teeracle::schedule_periodic::schedule_periodic;

use itp_types::parentchain::Hash;
use log::*;
use std::time::Duration;

pub(crate) mod schedule_periodic;
pub(crate) mod teeracle_metrics;

/// Schedule periodic reregistration of the enclave.
///
/// The `send_register_xt` needs to create a fresh registration extrinsic every time it is called
/// (updated nonce, fresh IAS-RA or DCAP-Quote).
///
/// Currently, this is only used for the teeracle, but could also be used for other flavors in the
/// future.
pub(crate) fn schedule_periodic_reregistration_thread(
	send_register_xt: impl Fn() -> Option<Hash> + std::marker::Send + 'static,
	period: Duration,
) {
	println!("Schedule periodic enclave reregistration every: {:?}", period);

	std::thread::Builder::new()
		.name("enclave_reregistration_thread".to_owned())
		.spawn(move || {
			schedule_periodic(
				|| {
					trace!("Reregistering the enclave.");
					if let Some(block_hash) = send_register_xt() {
						println!(
							"✅ Successfully reregistered the enclave. Block hash: {}.",
							block_hash
						)
					} else {
						error!("❌ Could not reregister the enclave.")
					}
				},
				period,
			);
		})
		.unwrap();
}
