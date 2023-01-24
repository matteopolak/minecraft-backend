use database::Status;

pub trait Submit {
	fn submit(
		&self,
		username: &str,
		status: Status,
	) -> Result<(bool, f64), Box<dyn std::error::Error>>;
}

pub trait HighPrioritySource {
	fn next_high(&mut self) -> Option<String>;
}

pub trait MediumPrioritySource {
	fn next_medium(&mut self) -> Option<String>;
}

pub trait LowPrioritySource {
	fn next_low(&mut self) -> Option<String>;
}

pub trait Connector:
	HighPrioritySource + MediumPrioritySource + LowPrioritySource + Submit
{
	fn reset(&self) -> Result<(), Box<dyn std::error::Error>>;
	fn get_accounts(&self) -> Result<Vec<crate::account::Account>, Box<dyn std::error::Error>>;
	fn get_proxies(&self) -> Result<Vec<reqwest::Proxy>, Box<dyn std::error::Error>>;
}
