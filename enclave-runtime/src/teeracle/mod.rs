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
	error::{Error, Result},
	initialization::global_components::GLOBAL_OCALL_API_COMPONENT,
	utils::{
		get_extrinsic_factory_from_solo_or_parachain,
		get_node_metadata_repository_from_solo_or_parachain,
	},
};
use codec::{Decode, Encode};
use core::slice;
use ita_oracle::{
	create_coin_gecko_oracle, create_coin_market_cap_oracle, create_open_meteo_weather_oracle,
	metrics_exporter::ExportMetrics,
	oracles::{
		exchange_rate_oracle::{ExchangeRateOracle, GetExchangeRate},
		weather_oracle::{GetLongitude, WeatherOracle},
	},
	traits::OracleSource,
	types::{TradingInfo, TradingPair, WeatherInfo, WeatherQuery},
};
use itp_component_container::ComponentGetter;
use itp_extrinsics_factory::CreateExtrinsics;
use itp_node_api::metadata::{pallet_teeracle::TeeracleCallIndexes, provider::AccessNodeMetadata};
use itp_types::OpaqueCall;
use itp_utils::write_slice_and_whitespace_pad;
use log::*;
use sgx_types::sgx_status_t;
use sp_runtime::OpaqueExtrinsic;
use std::{string::String, vec::Vec};

fn update_weather_data_internal(weather_info: WeatherInfo) -> Result<Vec<OpaqueExtrinsic>> {
	let extrinsics_factory = get_extrinsic_factory_from_solo_or_parachain()?;
	let ocall_api = GLOBAL_OCALL_API_COMPONENT.get()?;

	let mut extrinsic_calls: Vec<OpaqueCall> = Vec::new();

	let open_meteo_weather_oracle = create_open_meteo_weather_oracle(ocall_api);

	match get_longitude(weather_info, open_meteo_weather_oracle) {
		Ok(opaque_call) => extrinsic_calls.push(opaque_call),
		Err(e) => {
			error!("[-] Failed to get the newest longitude from OpenMeteo. {:?}", e);
		},
	};
	let extrinsics = extrinsics_factory.create_extrinsics(extrinsic_calls.as_slice(), None)?;
	Ok(extrinsics)
}

fn get_longitude<OracleSourceType, MetricsExporter>(
	weather_info: WeatherInfo,
	oracle: WeatherOracle<OracleSourceType, MetricsExporter>,
) -> Result<OpaqueCall>
where
	OracleSourceType: OracleSource<
		WeatherInfo,
		OracleRequestResult = std::result::Result<f32, ita_oracle::error::Error>,
	>,
	MetricsExporter: ExportMetrics<WeatherInfo>,
{
	let longitude =
		oracle.get_longitude(weather_info.clone()).map_err(|e| Error::Other(e.into()))?;

	let base_url = oracle.get_base_url().map_err(|e| Error::Other(e.into()))?;
	let source_base_url = base_url.as_str();

	println!("Update the longitude:  {}, for source {}", longitude, source_base_url);

	let node_metadata_repository = get_node_metadata_repository_from_solo_or_parachain()?;

	let call_ids = node_metadata_repository
		.get_from_metadata(|m| m.update_oracle_call_indexes())
		.map_err(Error::NodeMetadataProvider)?
		.map_err(|e| Error::Other(format!("{:?}", e).into()))?;

	let call = OpaqueCall::from_tuple(&(
		call_ids,
		weather_info.weather_query.key().as_bytes().to_vec(),
		source_base_url.as_bytes().to_vec(),
		longitude.encode(),
	));

	Ok(call)
}

#[no_mangle]
pub unsafe extern "C" fn update_weather_data_xt(
	weather_info_longitude: *const u8,
	weather_info_longitude_size: u32,
	weather_info_latitude: *const u8,
	weather_info_latitude_size: u32,
	unchecked_extrinsic: *mut u8,
	unchecked_extrinsic_size: u32,
) -> sgx_status_t {
	let mut weather_info_longitude_slice =
		slice::from_raw_parts(weather_info_longitude, weather_info_longitude_size as usize);
	let longitude = match String::decode(&mut weather_info_longitude_slice) {
		Ok(val) => val,
		Err(e) => {
			error!("Could not decode longitude: {:?}", e);
			return sgx_status_t::SGX_ERROR_UNEXPECTED
		},
	};

	let mut weather_info_latitude_slice =
		slice::from_raw_parts(weather_info_latitude, weather_info_latitude_size as usize);
	let latitude = match String::decode(&mut weather_info_latitude_slice) {
		Ok(val) => val,
		Err(e) => {
			error!("Could not decode latitude: {:?}", e);
			return sgx_status_t::SGX_ERROR_UNEXPECTED
		},
	};

	let weather_query = WeatherQuery { longitude, latitude, hourly: " ".into() };
	let weather_info = WeatherInfo { weather_query };

	let extrinsics = match update_weather_data_internal(weather_info) {
		Ok(xts) => xts,
		Err(e) => {
			error!("Updating weather info failed: {:?}", e);
			return sgx_status_t::SGX_ERROR_UNEXPECTED
		},
	};

	let extrinsic_slice =
		slice::from_raw_parts_mut(unchecked_extrinsic, unchecked_extrinsic_size as usize);

	// Save created extrinsic as slice in the return value unchecked_extrinsic.
	if let Err(e) = write_slice_and_whitespace_pad(extrinsic_slice, extrinsics.encode()) {
		error!("Copying encoded extrinsics into return slice failed: {:?}", e);
		return sgx_status_t::SGX_ERROR_UNEXPECTED
	}

	sgx_status_t::SGX_SUCCESS
}

/// For now get the crypto/fiat currency exchange rate from coingecko and CoinMarketCap.
#[no_mangle]
pub unsafe extern "C" fn update_market_data_xt(
	crypto_currency_ptr: *const u8,
	crypto_currency_size: u32,
	fiat_currency_ptr: *const u8,
	fiat_currency_size: u32,
	unchecked_extrinsic: *mut u8,
	unchecked_extrinsic_size: u32,
) -> sgx_status_t {
	let mut crypto_currency_slice =
		slice::from_raw_parts(crypto_currency_ptr, crypto_currency_size as usize);
	let crypto_currency: String = Decode::decode(&mut crypto_currency_slice).unwrap();

	let mut fiat_currency_slice =
		slice::from_raw_parts(fiat_currency_ptr, fiat_currency_size as usize);
	let fiat_currency: String = Decode::decode(&mut fiat_currency_slice).unwrap();

	let extrinsics = match update_market_data_internal(crypto_currency, fiat_currency) {
		Ok(xts) => xts,
		Err(e) => {
			error!("Update market data failed: {:?}", e);
			return sgx_status_t::SGX_ERROR_UNEXPECTED
		},
	};

	if extrinsics.is_empty() {
		error!("Updating market data yielded no extrinsics");
		return sgx_status_t::SGX_ERROR_UNEXPECTED
	}
	let extrinsic_slice =
		slice::from_raw_parts_mut(unchecked_extrinsic, unchecked_extrinsic_size as usize);

	// Save created extrinsic as slice in the return value unchecked_extrinsic.
	if let Err(e) = write_slice_and_whitespace_pad(extrinsic_slice, extrinsics.encode()) {
		error!("Copying encoded extrinsics into return slice failed: {:?}", e);
		return sgx_status_t::SGX_ERROR_UNEXPECTED
	}

	sgx_status_t::SGX_SUCCESS
}

fn update_market_data_internal(
	crypto_currency: String,
	fiat_currency: String,
) -> Result<Vec<OpaqueExtrinsic>> {
	let extrinsics_factory = get_extrinsic_factory_from_solo_or_parachain()?;
	let ocall_api = GLOBAL_OCALL_API_COMPONENT.get()?;

	let mut extrinsic_calls: Vec<OpaqueCall> = Vec::new();

	// Get the exchange rate
	let trading_pair = TradingPair { crypto_currency, fiat_currency };

	let coin_gecko_oracle = create_coin_gecko_oracle(ocall_api.clone());

	match get_exchange_rate(trading_pair.clone(), coin_gecko_oracle) {
		Ok(opaque_call) => extrinsic_calls.push(opaque_call),
		Err(e) => {
			error!("[-] Failed to get the newest exchange rate from CoinGecko. {:?}", e);
		},
	};

	let coin_market_cap_oracle = create_coin_market_cap_oracle(ocall_api);
	match get_exchange_rate(trading_pair, coin_market_cap_oracle) {
		Ok(oc) => extrinsic_calls.push(oc),
		Err(e) => {
			error!("[-] Failed to get the newest exchange rate from CoinMarketCap. {:?}", e);
		},
	};

	let extrinsics = extrinsics_factory.create_extrinsics(extrinsic_calls.as_slice(), None)?;
	Ok(extrinsics)
}

fn get_exchange_rate<OracleSourceType, MetricsExporter>(
	trading_pair: TradingPair,
	oracle: ExchangeRateOracle<OracleSourceType, MetricsExporter>,
) -> Result<OpaqueCall>
where
	OracleSourceType: OracleSource<TradingInfo>,
	MetricsExporter: ExportMetrics<TradingInfo>,
{
	let (rate, base_url) = oracle
		.get_exchange_rate(trading_pair.clone())
		.map_err(|e| Error::Other(e.into()))?;

	let source_base_url = base_url.as_str();

	println!(
		"Update the exchange rate:  {} = {:?} for source {}",
		trading_pair.clone().key(),
		rate,
		source_base_url,
	);

	let node_metadata_repository = get_node_metadata_repository_from_solo_or_parachain()?;

	let call_ids = node_metadata_repository
		.get_from_metadata(|m| m.update_exchange_rate_call_indexes())
		.map_err(Error::NodeMetadataProvider)?
		.map_err(|e| Error::Other(format!("{:?}", e).into()))?;

	let call = OpaqueCall::from_tuple(&(
		call_ids,
		source_base_url.as_bytes().to_vec(),
		trading_pair.key().as_bytes().to_vec(),
		Some(rate),
	));

	Ok(call)
}

/// For now get the crypto/fiat currency exchange rate from coingecko and CoinMarketCap.
#[no_mangle]
pub unsafe extern "C" fn send_nostr_test_message(
	unchecked_extrinsic: *mut u8,
	unchecked_extrinsic_size: u32,
) -> sgx_status_t {
	println!("calling working nostr function");
	nostr_test_works();
	println!("call finished working nostr function");

	//nostr_test_internal();
	sgx_status_t::SGX_SUCCESS
}

fn nostr_test_works() -> Result<()> {
	use nostr::prelude::*;
	use tungstenite_sgx as tungstenite;

	use nostr::{
		key::FromSkStr,
		nips::nip19::{FromBech32, ToBech32},
		types::{Metadata as NostrMetadata, Timestamp as NostrTimestamp},
		ChannelId, ClientMessage, Event, EventBuilder, EventId, Keys,
	};

	use itp_time_utils::now_as_secs;
	use secp256k1::Secp256k1;
	use tungstenite::Message as WsMessage;

	let secp = Secp256k1::new();

	// Generate new random keys
	//let my_keys = Keys::generate();

	// or use your already existing
	//
	// From HEX or Bech32
	let my_keys =
		Keys::from_sk_str("nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r", &secp)
			.unwrap();

	// Show bech32 public key
	let bech32_pubkey: String = my_keys.public_key().to_bech32().unwrap();
	println!("Bech32 PubKey: {}", bech32_pubkey);
	println!("Secret key: {}", my_keys.secret_key().unwrap().to_bech32().unwrap());

	let metadata = NostrMetadata::new()
		.name("somediddelidoo")
		.display_name("Some Diddelidoo")
		.about("I'm just testing");

	println!("metadata is: {:#?}", &metadata);

	let timestamp = NostrTimestamp::from(now_as_secs());
	println!("timestamp is: {:#?}", &timestamp);

	let event: Event = EventBuilder::set_metadata(metadata)
		.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
		.unwrap();
	println!("event is: {:#?}", &event);

	// New text note
	let timestamp = NostrTimestamp::from(now_as_secs());
	let event: Event = EventBuilder::new_text_note("Hello from Nostr SDK - in enclave", &[])
		.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
		.unwrap();
	println!("event is: {:#?}", &event);

	// Connect to relay
	let (mut socket, response) =
		tungstenite::connect("wss://nostr.lu.ke").expect("Can't connect to relay");

	println!("response is: {:#?}", &response);
	println!("socket is: {:#?}", &socket);

	println!("sending text message with id {}", event.id.to_bech32().unwrap());

	// Send msg
	let msg = ClientMessage::new_event(event).as_json();
	socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");

	/*
		// create channel
		let metadata = Metadata::new()
			.name("diddelichannel")
			.about("I'm just testing")
			.picture(Url::parse("https://placekitten.com/200/200")?);

		let event: Event = EventBuilder::new_channel(metadata)?.to_event(&my_keys)?;

		println!("creating channel with ID {}", event.id.to_bech32()?);
		let msg = ClientMessage::new_event(event).as_json();

		socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
	*/
	let channel_id = ChannelId::from(
		EventId::from_bech32("note18kst54gwje8n5t3cfpdud4duwh37wtfu4zpefd6a6q24nc2uecqs6vy8lq")
			.unwrap(),
	);

	println!("posting a message to channel {}", channel_id);

	let timestamp = NostrTimestamp::from(now_as_secs());
	let event: Event = EventBuilder::new_channel_msg(
		channel_id,
		Url::parse("wss://relay.damus.io").unwrap(),
		"post in channel",
	)
	.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
	.unwrap();

	let msg = ClientMessage::new_event(event).as_json();

	socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
	Ok(())
}

fn nostr_test_internal() -> Result<()> {
	use itp_time_utils::now_as_secs;
	use nostr::{
		key::FromSkStr,
		nips::nip19::{FromBech32, ToBech32},
		prelude::*,
		types::Timestamp as NostrTimestamp,
		ChannelId, ClientMessage, Event, EventBuilder, EventId, Keys,
	};
	use secp256k1::Secp256k1;
	use tungstenite::Message as WsMessage;
	use tungstenite_sgx as tungstenite;
	// Generate new random keys
	//let my_keys = Keys::generate();

	// or use your already existing
	//
	// From HEX or Bech32
	let secp = Secp256k1::new();
	let my_keys =
		Keys::from_sk_str("nsec13wqyx0syeu7unce6d7p8x4rqqe7elpfpr9ywsl5y6x427dzj8tyq36ku2r", &secp)
			.unwrap();

	// Show bech32 public key
	let bech32_pubkey: String = my_keys.public_key().to_bech32().unwrap();
	println!("Bech32 PubKey: {}", bech32_pubkey);
	println!("Secret key: {}", my_keys.secret_key().unwrap().to_bech32().unwrap());

	// use nostr::types::Metadata as NostrMetadata;
	// let metadata = NostrMetadata::new()
	// 	.name("somediddelidoo")
	// 	.display_name("Some Diddelidoo")
	// 	.about("I'm just testing");

	let timestamp = NostrTimestamp::from(now_as_secs());
	// let event: Event = EventBuilder::set_metadata(metadata)
	// 	.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
	// 	.unwrap();

	// New text note
	let event: Event = EventBuilder::new_text_note("Hello from Nostr SDK - inside enclave", &[])
		.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
		.unwrap();
	println!("event is: {:#?}", &event);

	// Connect to relay
	let (mut socket, response) =
		tungstenite::connect("wss://nostr.lu.ke").expect("Can't connect to relay");

	println!("response is: {:#?}", &response);
	println!("socket is: {:#?}", &socket);

	println!("sending text message with id {}", event.id.to_bech32().unwrap());

	// Send msg
	let msg = ClientMessage::new_event(event).as_json();
	println!("msg is: {}", &msg);
	socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");

	/*
		// create channel
		let metadata = Metadata::new()
			.name("diddelichannel")
			.about("I'm just testing")
			.picture(Url::parse("https://placekitten.com/200/200")?);
		let event: Event = EventBuilder::new_channel(metadata)?.to_event(&my_keys)?;
		println!("creating channel with ID {}", event.id.to_bech32()?);
		let msg = ClientMessage::new_event(event).as_json();

		socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");
	*/
	let channel_id = ChannelId::from(
		EventId::from_bech32("note18kst54gwje8n5t3cfpdud4duwh37wtfu4zpefd6a6q24nc2uecqs6vy8lq")
			.unwrap(),
	);

	println!("posting a message to channel {}", channel_id);

	let timestamp = NostrTimestamp::from(now_as_secs());
	let event: Event = EventBuilder::new_channel_msg(
		channel_id,
		Url::parse("wss://relay.damus.io").unwrap(),
		"post in channel",
	)
	.to_event_with_timestamp_with_secp(&my_keys, timestamp, &secp)
	.unwrap();

	let msg = ClientMessage::new_event(event).as_json();

	socket.write_message(WsMessage::Text(msg)).expect("Impossible to send message");

	Ok(())
}
