use api::{microsoft::JavaData, xbox::Credentials};
use reqwest::{Client, Proxy, StatusCode};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct Account {
	clients: Vec<Client>,
	proxies: Vec<Proxy>,
	credentials: Credentials,
	index: usize,
	token: Option<JavaData>,
}

#[derive(Debug)]
pub enum Error {
	NoClient,
	Token,
	Request,
	Deserialization,
	Retry,
	Delay(tokio::time::Duration),
}

#[derive(Deserialize)]
pub struct MinecraftResponse {
	pub status: String,
}

impl Account {
	pub fn new(username: String, password: String) -> Self {
		Self {
			credentials: Credentials { username, password },
			index: 0,
			clients: vec![],
			proxies: vec![],
			token: None,
		}
	}

	pub fn add_agent(&mut self, agent: Proxy) {
		self.clients.push(
			Client::builder()
				.proxy(agent.clone())
				.gzip(true)
				.build()
				.unwrap(),
		);

		self.proxies.push(agent);
	}

	pub fn remove_current_client(&mut self) {
		if self.clients.is_empty() {
			return;
		}

		self.clients.remove(self.index);
	}

	pub fn get_client(&mut self) -> Option<&Client> {
		if self.clients.is_empty() {
			return None;
		}

		self.index = (self.index + 1) % self.clients.len();
		let client = self.clients.get(self.index);

		client
	}

	pub fn get_client_and_credentials(&mut self) -> Option<(&Client, &Credentials)> {
		if self.clients.is_empty() {
			return None;
		}

		self.index = (self.index + 1) % self.clients.len();
		let client = self.clients.get(self.index);

		client.map(|client| (client, &self.credentials))
	}

	pub async fn update_token(&mut self) -> Result<JavaData, Error> {
		let Some((client, credentials)) = self.get_client_and_credentials() else {
			return Err(Error::NoClient);
		};

		api::microsoft::get_java_token(client, credentials)
			.await
			.map_err(|_| Error::Token)
	}

	pub fn is_token_valid(token: Option<&JavaData>) -> bool {
		match token {
			Some(token) => token.expires_at > chrono::Utc::now() + chrono::Duration::seconds(30),
			None => false,
		}
	}

	pub async fn check(&mut self, name: &str) -> Result<bool, Error> {
		let java = if Self::is_token_valid(self.token.as_ref()) {
			self.token.clone()
		} else {
			self.token = Some(self.update_token().await?);
			self.token.clone()
		};

		let Some(client) = self.get_client() else {
			return Err(Error::NoClient);
		};

		let response = match client
			.get(format!(
				"https://api.minecraftservices.com/minecraft/profile/name/{name}/available"
			))
			.header(
				reqwest::header::AUTHORIZATION,
				java.ok_or(Error::Token)?.token,
			)
			.send()
			.await
		{
			Ok(response) => response,
			Err(e) => {
				eprintln!("Error: {e:?}");
				eprintln!("Proxy: {:?}", self.proxies.get(self.index));
				self.remove_current_client();
				return Err(Error::Retry);
			}
		};

		if response.status() == StatusCode::PAYMENT_REQUIRED {
			self.remove_current_client();
			return Err(Error::Retry);
		}

		if response.status() == StatusCode::TOO_MANY_REQUESTS {
			return Err(Error::Delay(tokio::time::Duration::from_secs(30)));
		}

		if response.status() != StatusCode::OK {
			return Err(Error::Request);
		}

		Ok(response
			.json::<MinecraftResponse>()
			.await
			.map_err(|_| Error::Deserialization)?
			.status == "AVAILABLE")
	}
}
