use std::str::FromStr;

use reqwest::Client;
use serde::{Deserialize, Serialize};
// use serde_aux::prelude::deserialize_number_from_string;

#[derive(Debug)]
pub struct PreAuthData {
	cookie: String,
	ppft: String,
	url: String,
}

#[derive(Deserialize, Debug)]
pub struct LogUserResponse {
	access_token: String,
	// token_type: String,
	// #[serde(deserialize_with = "deserialize_number_from_string")]
	// expires_in: u64,
	// scope: String,
	// refresh_token: String,
	// user_id: String,
}

#[derive(Debug)]
pub struct Credentials<'a> {
	pub username: &'a str,
	pub password: &'a str,
}

#[derive(Serialize, Debug)]
pub struct LogUserQuery<'a> {
	login: &'a str,
	loginfmt: &'a str,
	passwd: &'a str,
	ppft: &'a str,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RpsTicketResponse {
	// issue_instant: String,
	// not_after: String,
	token: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RpsTicketPayloadProperties<'a> {
	auth_method: &'static str,
	site_name: &'static str,
	rps_ticket: &'a str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct RpsTicketPayload<'a> {
	relying_party: &'static str,
	token_type: &'static str,
	properties: &'a RpsTicketPayloadProperties<'a>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct XstsResponse {
	display_claims: XstsDisplayClaims,
	not_after: String,
	token: String,
}

#[derive(Deserialize, Debug)]
pub struct XstsDisplayClaims {
	xui: Vec<XstsXui>,
}

#[derive(Deserialize, Debug)]
pub struct XstsXui {
	uhs: String,
	xid: Option<String>,
}

#[derive(Debug)]
pub struct XstsData {
	pub xid: Option<String>,
	pub hash: String,
	pub token: String,
	pub expires_on: String,
}

#[derive(Debug)]
pub enum Error {
	ParseError,
	RequestError,
	SerializationError,
	DeserializationError,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct XstsPayloadProperties<'a> {
	user_tokens: &'a Vec<&'a str>,
	// proof_key: Option<&'a str>,
	sandbox_id: &'static str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct XstsPayload<'a> {
	relying_party: &'static str,
	token_type: &'static str,
	properties: &'a XstsPayloadProperties<'a>,
}

pub async fn pre_auth(client: &Client) -> Result<PreAuthData, Error> {
	let mut headers = reqwest::header::HeaderMap::new();

	headers.insert(
		reqwest::header::ACCEPT_ENCODING,
		reqwest::header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		reqwest::header::ACCEPT_LANGUAGE,
		reqwest::header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		reqwest::header::USER_AGENT,
		reqwest::header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
	);

	let response = client
		.get("https://login.live.com/oauth20_authorize.srf")
		.query(&[
			("client_id", "000000004C12AE6F"),
			("redirect_uri", "https://login.live.com/oauth20_desktop.srf"),
			("scope", "service::user.auth.xboxlive.com::MBI_SSL"),
			("display", "touch"),
			("response_type", "token"),
			("locale", "en"),
		])
		.headers(headers)
		.send()
		.await
		.map_err(|_| Error::RequestError)?;

	let cookie = response
		.headers()
		.get_all("set-cookie")
		.into_iter()
		.filter_map(|s| {
			if let Ok(string) = s.to_str() {
				string.split(';').next()
			} else {
				None
			}
		})
		.intersperse(";")
		.collect::<String>();

	let html = response.text().await.map_err(|_| Error::RequestError)?;

	Ok(PreAuthData {
		cookie,
		ppft: {
			let begin = if let Some(begin) = html.find("sFTTag:'") {
				html[begin..].find("value=\"").map(|b| b + begin + 7)
			} else {
				None
			};

			let end = if let Some(begin) = begin {
				html[begin..].find("\"/>'").map(|end| end + begin)
			} else {
				None
			};

			match (begin, end) {
				(Some(begin), Some(end)) => Some(html[begin..end].to_string()),
				_ => None,
			}
		}
		.ok_or(Error::ParseError)?,
		url: {
			let begin = html.find("urlPost:'").map(|begin| begin + 9);

			let end = if let Some(begin) = begin {
				html[begin..].find("'").map(|end| end + begin)
			} else {
				None
			};

			match (begin, end) {
				(Some(begin), Some(end)) => Some(html[begin..end].to_string()),
				_ => None,
			}
		}
		.ok_or(Error::ParseError)?,
	})
}

pub async fn log_user(
	client: &Client,
	auth: &PreAuthData,
	credentials: &Credentials<'_>,
) -> Result<LogUserResponse, Error> {
	let mut headers = reqwest::header::HeaderMap::new();

	headers.insert(
		reqwest::header::ACCEPT_ENCODING,
		reqwest::header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		reqwest::header::ACCEPT_LANGUAGE,
		reqwest::header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		reqwest::header::USER_AGENT,
		reqwest::header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
	);

	headers.insert(
		reqwest::header::CONTENT_TYPE,
		reqwest::header::HeaderValue::from_static("application/x-www-form-urlencoded"),
	);

	headers.insert(
		reqwest::header::COOKIE,
		reqwest::header::HeaderValue::from_str(&auth.cookie)
			.map_err(|_| Error::SerializationError)?,
	);

	let qs = serde_qs::to_string(&LogUserQuery {
		login: &credentials.username,
		loginfmt: &credentials.username,
		passwd: &credentials.password,
		ppft: &auth.ppft,
	})
	.map_err(|_| Error::SerializationError)?;

	let response = client
		.post(&auth.url)
		.body(qs)
		.headers(headers)
		.send()
		.await
		.map_err(|_| Error::RequestError)?;

	serde_qs::from_str::<LogUserResponse>(
		response
			.url()
			.fragment()
			.ok_or(Error::DeserializationError)?,
	)
	.map_err(|_| Error::DeserializationError)
}

pub async fn exchange_rps_ticket_for_token(
	client: &Client,
	ticket: &LogUserResponse,
) -> Result<RpsTicketResponse, Error> {
	let mut headers = reqwest::header::HeaderMap::new();

	headers.insert(
		reqwest::header::ACCEPT_ENCODING,
		reqwest::header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		reqwest::header::ACCEPT_LANGUAGE,
		reqwest::header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		reqwest::header::USER_AGENT,
		reqwest::header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
	);

	headers.insert(
		reqwest::header::ACCEPT,
		reqwest::header::HeaderValue::from_static("application/json"),
	);

	headers.insert(
		"x-xbl-contract-version",
		reqwest::header::HeaderValue::from_static("0"),
	);

	headers.insert(
		reqwest::header::CONTENT_TYPE,
		reqwest::header::HeaderValue::from_static("application/json"),
	);

	let response = client
		.post("https://user.auth.xboxlive.com/user/authenticate")
		.json(&RpsTicketPayload {
			relying_party: "http://auth.xboxlive.com",
			token_type: "JWT",
			properties: &RpsTicketPayloadProperties {
				auth_method: "RPS",
				site_name: "user.auth.xboxlive.com",
				rps_ticket: &ticket.access_token,
			},
		})
		.headers(headers)
		.send()
		.await
		.map_err(|_| Error::RequestError)?;

	response
		.json::<RpsTicketResponse>()
		.await
		.map_err(|_| Error::DeserializationError)
}

pub fn sign<'a>(url: &str, payload: &str) -> Result<Vec<u8>, Error> {
	let rng = ring::rand::SystemRandom::new();

	// we only use this function once per key, so we can just generate it here
	// generate an EC P-256 keypair with ring
	let keypair = ring::signature::EcdsaKeyPair::generate_pkcs8(
		&ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
		&rng,
	)
	.map_err(|_| Error::SerializationError)?;

	// get the EcdsaKeyPair
	let keypair = ring::signature::EcdsaKeyPair::from_pkcs8(
		&ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
		keypair.as_ref(),
	)
	.map_err(|_| Error::SerializationError)?;

	let windows_timestamp = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.expect("Time went backwards")
		.as_secs();

	let windows_timestamp = (windows_timestamp + 11_644_473_600) * 10_000_000;
	let path = reqwest::Url::from_str(url).map_err(|_| Error::SerializationError)?;
	let path = path.path();

	let size = /* sig */ 5 + /* ts */ 9 + /* POST */ 5 + path.len() + 1 + 1 + payload.len() + 1;
	let mut buf: Vec<u8> = Vec::with_capacity(size);

	buf.extend(1i32.to_be_bytes());
	buf.extend(0u8.to_be_bytes());
	buf.extend(windows_timestamp.to_be_bytes());
	buf.extend(0u8.to_be_bytes());
	buf.extend("POST".as_bytes());
	buf.extend(0u8.to_be_bytes());
	buf.extend(path.as_bytes());
	buf.extend(0u8.to_be_bytes());
	buf.extend(0u8.to_be_bytes());
	buf.extend(payload.as_bytes());
	buf.extend(0u8.to_be_bytes());

	// sign the buffer with SHA256, ieee-p1363 and using the private key from `keypair`
	let signature = keypair
		.sign(&rng, &buf)
		.map_err(|_| Error::SerializationError)?;

	buf.extend(1i32.to_be_bytes());
	buf.extend(windows_timestamp.to_be_bytes());

	// append signature to buffer
	buf.extend(signature.as_ref());

	Ok(buf)
}

pub async fn get_xsts_token(
	client: &Client,
	credentials: &Credentials<'_>,
) -> Result<XstsData, Error> {
	let pre_auth = pre_auth(client).await?;
	let log_user = log_user(client, &pre_auth, credentials).await?;
	let rps_ticket = exchange_rps_ticket_for_token(client, &log_user).await?;

	let payload = XstsPayload {
		relying_party: "rp://api.minecraftservices.com/",
		token_type: "JWT",
		properties: &XstsPayloadProperties {
			user_tokens: &vec![&rps_ticket.token],
			sandbox_id: "RETAIL",
			// proof_key: None,
		},
	};

	// let signature = sign(
	// 	"https://xsts.auth.xboxlive.com/xsts/authorize",
	// 	&serde_json::to_string(&payload).map_err(|_| Error::SerializationError)?,
	// )
	// .map_err(|_| Error::SerializationError)?;

	let mut headers = reqwest::header::HeaderMap::new();

	headers.insert(
		reqwest::header::CACHE_CONTROL,
		reqwest::header::HeaderValue::from_static("no-store, must-revalidate, no-cache"),
	);

	headers.insert(
		"x-xbl-contract-version",
		reqwest::header::HeaderValue::from_static("1"),
	);

	// let signature = base64::engine::general_purpose::STANDARD.encode(&signature);

	// headers.insert(
	// 	"Signature",
	// 	reqwest::header::HeaderValue::from_str(&signature)
	// 		.map_err(|_| Error::SerializationError)?,
	// );

	headers.insert(
		reqwest::header::CONTENT_TYPE,
		reqwest::header::HeaderValue::from_static("application/json"),
	);

	let response = client
		.post("https://xsts.auth.xboxlive.com/xsts/authorize")
		.json(&payload)
		.headers(headers)
		.send()
		.await
		.map_err(|_| Error::RequestError)?;

	let response = response
		.json::<XstsResponse>()
		.await
		.map_err(|_| Error::DeserializationError)?;

	Ok(XstsData {
		token: response.token,
		expires_on: response.not_after,
		xid: response.display_claims.xui[0].xid.clone(),
		hash: response.display_claims.xui[0].uhs.clone(),
	})
}
