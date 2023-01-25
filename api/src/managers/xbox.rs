use std::{fs::File, io::BufReader, path::Path, str::FromStr};

use reqwest::{
	header::{self, HeaderMap},
	Client,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone)]
pub struct Credentials {
	pub username: String,
	pub password: String,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct XstsData {
	pub xid: Option<String>,
	pub hash: String,
	pub token: String,
	pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub enum Error {
	ParseError,
	RequestError,
	SerializationError,
	DeserializationError,
	CacheError,
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

fn rps_ticker_headers() -> HeaderMap {
	let mut headers = HeaderMap::new();

	headers.insert(
		header::ACCEPT_ENCODING,
		header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		header::ACCEPT_LANGUAGE,
		header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		header::USER_AGENT,
		header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
	);

	headers.insert(
		header::ACCEPT,
		header::HeaderValue::from_static("application/json"),
	);

	headers.insert(
		"x-xbl-contract-version",
		header::HeaderValue::from_static("0"),
	);

	headers.insert(
		header::CONTENT_TYPE,
		header::HeaderValue::from_static("application/json"),
	);

	headers
}

/// # Errors
/// - `Error::RequestError` if the request fails
/// - `Error::DeserializationError` if the response cannot be deserialized
/// - `Error::ParseError` if the response is invalid
pub async fn pre_auth(client: &Client) -> Result<PreAuthData, Error> {
	let mut headers = HeaderMap::new();

	headers.insert(
		header::ACCEPT_ENCODING,
		header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		header::ACCEPT_LANGUAGE,
		header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		header::USER_AGENT,
		header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
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
				html[begin..].find('\'').map(|end| end + begin)
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

/// # Errors
/// - `Error::RequestError` if the request fails
/// - `Error::DeserializationError` if the response cannot be deserialized
/// - `Error::SerializationError` if the request cannot be serialized
/// - `Error::ParseError` if the response cannot be parsed
pub async fn log_user(
	client: &Client,
	auth: &PreAuthData,
	credentials: &Credentials,
) -> Result<LogUserResponse, Error> {
	let mut headers = HeaderMap::new();

	headers.insert(
		header::ACCEPT_ENCODING,
		header::HeaderValue::from_static("gzip"),
	);

	headers.insert(
		header::ACCEPT_LANGUAGE,
		header::HeaderValue::from_static("en-US"),
	);

	headers.insert(
		header::USER_AGENT,
		header::HeaderValue::from_static("Mozilla/5.0 (XboxReplay; XboxLiveAuth/3.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/71.0.3578.98 Safari/537.36"),
	);

	headers.insert(
		header::CONTENT_TYPE,
		header::HeaderValue::from_static("application/x-www-form-urlencoded"),
	);

	headers.insert(
		header::COOKIE,
		header::HeaderValue::from_str(&auth.cookie).map_err(|_| Error::SerializationError)?,
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

/// # Errors
/// - `Error::RequestError` if the request fails
/// - `Error::DeserializationError` if the response cannot be deserialized
pub async fn exchange_rps_ticket_for_token(
	client: &Client,
	ticket: &LogUserResponse,
) -> Result<RpsTicketResponse, Error> {
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
		.headers(rps_ticker_headers())
		.send()
		.await
		.map_err(|_| Error::RequestError)?;

	response
		.json::<RpsTicketResponse>()
		.await
		.map_err(|_| Error::DeserializationError)
}

/// # Errors
/// - `Error::RequestError` if the request fails
/// - `Error::DeserializationError` if the response cannot be deserialized
pub async fn get_xsts_token(
	client: &Client,
	credentials: &Credentials,
	cache: Option<&Path>,
) -> Result<XstsData, Error> {
	if let Some((cache, true)) = cache.map(|cache| (cache, cache.is_dir())) {
		let mut cache = cache.to_path_buf();
		cache.push(&credentials.username);
		cache.push("xsts.json");

		if cache.is_file() {
			let file = File::open(cache).map_err(|_| Error::CacheError)?;
			let reader = BufReader::new(file);

			let data = serde_json::from_reader::<_, XstsData>(reader)
				.map_err(|_| Error::DeserializationError)?;

			if data.expires_at > chrono::Utc::now() + chrono::Duration::minutes(5) {
				return Ok(data);
			}
		}
	}

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

	let mut headers = HeaderMap::new();

	headers.insert(
		header::CACHE_CONTROL,
		header::HeaderValue::from_static("no-store, must-revalidate, no-cache"),
	);

	headers.insert(
		"x-xbl-contract-version",
		header::HeaderValue::from_static("1"),
	);

	headers.insert(
		header::CONTENT_TYPE,
		header::HeaderValue::from_static("application/json"),
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

	let data = XstsData {
		token: response.token,
		expires_at: chrono::DateTime::<chrono::Utc>::from_str(&response.not_after)
			.map_err(|_| Error::DeserializationError)?,
		xid: response.display_claims.xui[0].xid.clone(),
		hash: response.display_claims.xui[0].uhs.clone(),
	};

	if let Some(cache) = cache {
		let mut cache = cache.to_path_buf();
		cache.push(&credentials.username);

		if !cache.is_dir() {
			std::fs::create_dir_all(&cache).map_err(|_| Error::CacheError)?;
		}

		cache.push("xsts.json");

		let file = File::create(cache).map_err(|_| Error::CacheError)?;

		serde_json::to_writer(file, &data).map_err(|_| Error::SerializationError)?;
	}

	Ok(data)
}
