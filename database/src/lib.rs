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
