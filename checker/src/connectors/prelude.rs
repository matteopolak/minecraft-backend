use database::{models::Snipe, Status};

pub trait Submit {
	async fn submit(
		&self,
		username: &str,
		status: Status,
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
	fn reset(&self) -> Result<(), Box<dyn std::error::Error>>;
	fn get_accounts<'a>(
		&self,
	) -> Result<Vec<crate::account::Account<'a>>, Box<dyn std::error::Error>>;
	fn get_proxies(&self) -> Result<Vec<reqwest::Proxy>, Box<dyn std::error::Error>>;
	async fn check_for_snipe(&mut self) -> Option<&Snipe>;
}
