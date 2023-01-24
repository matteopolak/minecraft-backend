use database::{schema, PostgresPool};
use diesel::{
	dsl::sql, sql_types::Timestamptz, BoolExpressionMethods, ExpressionMethods, IntoSql, QueryDsl,
	Queryable, RunQueryDsl,
};

use crate::{account::Account, connectors::prelude::*};

pub struct Postgres {
	high: Vec<String>,
	medium: Vec<String>,
	low: Vec<String>,
	pool: PostgresPool,
}

#[derive(Queryable)]
pub struct AccountData {
	username: String,
	password: String,
}

#[derive(Queryable)]
pub struct ProxyData {
	address: String,
	port: i32,
	username: Option<String>,
	password: Option<String>,
}

impl Postgres {
	pub fn new(pool: PostgresPool) -> Self {
		Self {
			high: Vec::new(),
			medium: Vec::new(),
			low: Vec::new(),
			pool,
		}
	}
}

impl Connector for Postgres {
	fn get_accounts(&self) -> Result<Vec<Account>, Box<dyn std::error::Error>> {
		let accounts = schema::accounts::table
			.select((schema::accounts::username, schema::accounts::password))
			.load::<AccountData>(&mut self.pool.get()?)?;

		Ok(accounts
			.into_iter()
			.map(|row| {
				// we can leak these strings because they will live for the duration of the program
				Account::new(row.username, row.password)
			})
			.collect())
	}

	fn get_proxies(&self) -> Result<Vec<reqwest::Proxy>, Box<dyn std::error::Error>> {
		let proxies = schema::proxies::table
			.select((
				schema::proxies::address,
				schema::proxies::port,
				schema::proxies::username,
				schema::proxies::password,
			))
			.load::<ProxyData>(&mut self.pool.get()?)?;

		Ok(proxies
			.into_iter()
			.filter_map(|row| {
				// we can leak these strings because they will live for the duration of the program
				match (row.username, row.password) {
					(Some(username), Some(password)) => Some(
						reqwest::Proxy::https(format!("{}:{}", row.address, row.port,))
							.ok()?
							.basic_auth(&username, &password),
					),
					_ => reqwest::Proxy::https(format!("{}:{}", row.address, row.port,)).ok(),
				}
			})
			.collect())
	}
}

impl Submit for Postgres {
	fn submit(
		&self,
		username: &str,
		available: bool,
	) -> Result<(bool, f64), Box<dyn std::error::Error>> {
		let conditional_update = sql::<Timestamptz>(&format!(
			"CASE WHEN (\"valid\" IS NULL OR \"valid\" = {}) THEN NOW() ELSE \"updatedAt\" END",
			!available
		))
		.into_sql();

		let row = diesel::update(schema::names::table)
			.filter(schema::names::username.eq(username))
			.set((
				schema::names::verified_at.eq(diesel::dsl::now),
				schema::names::updated_at.eq(conditional_update),
				schema::names::valid.eq(available),
				schema::names::available.eq(available),
				schema::names::updating.eq(false),
			))
			.returning((
				schema::names::frequency,
				schema::names::valid
					.eq(!available)
					.or(schema::names::valid.is_null()),
			))
			.get_result::<(f64, Option<bool>)>(&mut self.pool.get()?)?;

		Ok((row.1 != Some(false), row.0))
	}
}

impl HighPrioritySource for Postgres {
	fn next_high(&mut self) -> Option<String> {
		if self.high.is_empty() {
			let usernames = schema::names::table
				.filter(schema::names::updating.eq(false))
				.filter(schema::names::frequency.ge(15.))
				.order((
					schema::names::verified_at.asc(),
					schema::names::frequency.desc(),
				))
				.limit(100)
				.select(schema::names::username)
				.into_boxed();

			self.high = diesel::update(schema::names::table)
				.filter(schema::names::username.eq_any(usernames))
				.set(schema::names::updating.eq(true))
				.returning(schema::names::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.high.pop()
	}
}

impl MediumPrioritySource for Postgres {
	fn next_medium(&mut self) -> Option<String> {
		if self.medium.is_empty() {
			let usernames = schema::names::table
				.filter(schema::names::updating.eq(false))
				.filter(schema::names::frequency.ge(0.01))
				.filter(schema::names::frequency.lt(15.))
				.filter(schema::names::available.eq(true))
				.order((
					schema::names::verified_at.asc(),
					schema::names::frequency.desc(),
				))
				.limit(100)
				.select(schema::names::username)
				.into_boxed();

			self.medium = diesel::update(schema::names::table)
				.filter(schema::names::username.eq_any(usernames))
				.set(schema::names::updating.eq(true))
				.returning(schema::names::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.medium.pop()
	}
}

impl LowPrioritySource for Postgres {
	fn next_low(&mut self) -> Option<String> {
		if self.low.is_empty() {
			let usernames = schema::names::table
				.filter(schema::names::updating.eq(false))
				.filter(schema::names::available.eq(true))
				.filter(schema::names::frequency.lt(0.01))
				.filter(
					schema::names::frequency
						.ge(0.001)
						.or(schema::names::definition.is_not_null()),
				)
				.order((
					schema::names::verified_at.asc(),
					schema::names::frequency.desc(),
				))
				.limit(100)
				.select(schema::names::username)
				.into_boxed();

			self.low = diesel::update(schema::names::table)
				.filter(schema::names::username.eq_any(usernames))
				.set(schema::names::updating.eq(true))
				.returning(schema::names::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.low.pop()
	}
}
