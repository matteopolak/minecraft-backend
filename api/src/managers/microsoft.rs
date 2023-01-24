use std::{fs::File, io::BufReader, path::Path};

use crate::managers::xbox;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JavaData {
	pub token: String,
	pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JavaPayload<'a> {
	identity_token: &'a str,
}

#[derive(Deserialize, Debug)]
pub struct JavaResponse {
	access_token: String,
	token_type: String,
	expires_in: u32,
}

/// # Errors
/// - `xbox::Error::RequestError` if the request fails
/// - `xbox::Error::DeserializationError` if the response cannot be deserialized
pub async fn get_java_token(
	client: &Client,
	credentials: &xbox::Credentials,
	cache: Option<&Path>,
) -> Result<JavaData, xbox::Error> {
	if let Some((cache, true)) = cache.map(|cache| (cache, cache.is_dir())) {
		let mut cache = cache.to_path_buf();
		cache.push(&credentials.username);
		cache.push("java.json");

		if cache.is_file() {
			let file = File::open(cache).map_err(|_| xbox::Error::CacheError)?;
			let reader = BufReader::new(file);

			let data = serde_json::from_reader::<_, JavaData>(reader)
				.map_err(|_| xbox::Error::DeserializationError)?;

			if data.expires_at > chrono::Utc::now() {
				return Ok(data);
			}
		}
	}

	let xsts = xbox::get_xsts_token(client, credentials, cache).await?;

	let response = client
		.post("https://api.minecraftservices.com/authentication/login_with_xbox")
		.header(reqwest::header::CONTENT_TYPE, "application/json")
		.header(reqwest::header::USER_AGENT, "MinecraftLauncher/2.2.10675")
		.json(&JavaPayload {
			identity_token: &format!("XBL3.0 x={};{}", xsts.hash, xsts.token),
		})
		.send()
		.await
		.map_err(|_| xbox::Error::RequestError)?;

	let response = response
		.json::<JavaResponse>()
		.await
		.map_err(|_| xbox::Error::DeserializationError)?;

	let data = JavaData {
		token: format!("{} {}", response.token_type, response.access_token),
		expires_at: chrono::Utc::now() + chrono::Duration::seconds(i64::from(response.expires_in)),
	};

	if let Some(cache) = cache {
		let mut cache = cache.to_path_buf();
		cache.push(&credentials.username);

		if !cache.is_dir() {
			std::fs::create_dir_all(&cache).map_err(|_| xbox::Error::CacheError)?;
		}

		cache.push("java.json");

		let file = File::create(cache).map_err(|_| xbox::Error::CacheError)?;

		serde_json::to_writer(file, &data).map_err(|_| xbox::Error::SerializationError)?;
	}

	Ok(data)
}
