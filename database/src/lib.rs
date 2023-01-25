use std::str::FromStr;

use diesel::{
	r2d2::{ConnectionManager, Pool},
	PgConnection,
};

pub mod models;
pub mod schema;

pub fn get_pool() -> PostgresPool {
	let url = std::env::var("DATABASE_URL").expect("environment variable DATABASE_URL not found");
	let manager = ConnectionManager::<PgConnection>::new(url);

	Pool::builder()
		.build(manager)
		.expect("failed to create connection pool")
}

pub type PostgresPool = Pool<ConnectionManager<PgConnection>>;

#[derive(PartialEq)]
pub enum Status {
	Unknown,
	Available,
	Taken,
	Banned,
	BatchAvailable,
}

impl From<i16> for Status {
	fn from(status: i16) -> Self {
		match status {
			0 => Status::Unknown,
			1 => Status::Available,
			2 => Status::Taken,
			3 => Status::Banned,
			4 => Status::BatchAvailable,
			_ => Status::Unknown,
		}
	}
}

impl From<Status> for i16 {
	fn from(status: Status) -> Self {
		match status {
			Status::Unknown => 0,
			Status::Available => 1,
			Status::Taken => 2,
			Status::Banned => 3,
			Status::BatchAvailable => 4,
		}
	}
}

impl FromStr for Status {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"AVAILABLE" => Ok(Self::Available),
			"DUPLICATE" => Ok(Self::Taken),
			"NOT_ALLOWED" => Ok(Self::Banned),
			_ => Ok(Self::Unknown),
		}
	}
}
