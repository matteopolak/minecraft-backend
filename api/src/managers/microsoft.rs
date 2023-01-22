use crate::managers::xbox;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
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
	expires_in: u64,
}

pub async fn get_java_token(
	client: &Client,
	credentials: &xbox::Credentials<'_>,
) -> Result<JavaData, xbox::Error> {
	let xsts = xbox::get_xsts_token(client, credentials).await?;

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

	Ok(JavaData {
		token: format!("{} {}", response.token_type, response.access_token),
		expires_at: chrono::Utc::now() + chrono::Duration::seconds(response.expires_in as i64),
	})
}
