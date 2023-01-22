pub trait Submit {
	async fn submit(
		&self,
		username: &str,
		available: bool,
	) -> Result<(bool, f64), Box<dyn std::error::Error>>;
}

pub trait HighPrioritySource {
	async fn next_high(&mut self) -> Option<String>;
}

pub trait MediumPrioritySource {
	async fn next_medium(&mut self) -> Option<String>;
}

pub trait LowPrioritySource {
	async fn next_low(&mut self) -> Option<String>;
}

pub trait Connector:
	HighPrioritySource + MediumPrioritySource + LowPrioritySource + Submit
{
	async fn prepare() -> Result<Self, Box<dyn std::error::Error>>
	where
		Self: Sized;
	async fn get_accounts(
		&self,
	) -> Result<Vec<crate::account::Account>, Box<dyn std::error::Error>>;
	async fn get_proxies(&self) -> Result<Vec<reqwest::Proxy>, Box<dyn std::error::Error>>;
}
