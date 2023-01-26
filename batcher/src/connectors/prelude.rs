pub trait Connector {
	fn next(&mut self, size: i64) -> Option<Vec<String>>;
	fn submit_available(&self, names: Vec<String>) -> Result<(), Box<dyn std::error::Error>>;
	fn submit_unavailable(&self, names: Vec<String>) -> Result<(), Box<dyn std::error::Error>>;
}
