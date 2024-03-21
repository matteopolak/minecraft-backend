use api::microsoft::JavaData;
use database::{functions::date_trunc, models::Snipe, schema, PostgresPool, Status};
use diesel::{
	dsl::sql, sql_types::Timestamptz, BoolExpressionMethods, ExpressionMethods, IntoSql, QueryDsl,
	Queryable, RunQueryDsl,
};
use once_cell::sync::Lazy;
use reqwest::Client;
use tokio::sync::Mutex;

use crate::{
	account::{Account, CACHE_DIR},
	connectors::prelude::*,
	time,
};

pub struct Postgres {
	high: Vec<String>,
	medium: Vec<String>,
	low: Vec<String>,
	snipe: Option<Snipe>,
	snipe_token: Option<JavaData>,
	pool: PostgresPool,
	client: Option<Client>,
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

static SNIPE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

impl Postgres {
	pub fn new(pool: PostgresPool, client: Option<Client>) -> Self {
		Self {
			high: Vec::new(),
			medium: Vec::new(),
			low: Vec::new(),
			pool,
			snipe: None,
			snipe_token: None,
			client,
		}
	}
}

impl Connector for Postgres {
	fn reset(&self) -> Result<(), Box<dyn std::error::Error>> {
		diesel::update(schema::name::table)
			.set(schema::name::updating.eq(false))
			.execute(&mut self.pool.get()?)?;

		diesel::update(schema::snipe::table)
			.set(schema::snipe::count.eq(0))
			.execute(&mut self.pool.get()?)?;

		Ok(())
	}

	fn get_accounts<'a>(&self) -> Result<Vec<Account<'a>>, Box<dyn std::error::Error>> {
		let accounts = schema::account::table
			.select((schema::account::username, schema::account::password))
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
		let proxies = schema::proxy::table
			.select((
				schema::proxy::address,
				schema::proxy::port,
				schema::proxy::username,
				schema::proxy::password,
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

	async fn check_for_snipe(&mut self) -> Option<&Snipe> {
		if let Some(snipe) = self.snipe.as_ref() {
			if let Some(token) = self.snipe_token.as_ref() {
				if token.expires_at
					< chrono::Utc::now()
						+ chrono::Duration::try_minutes(5)
							.expect("5 minutes to be less than i64::MAX / 1_000")
				{
					self.snipe_token = None;
				}
			}

			if self.snipe_token.is_none() {
				// only allow one token request at a time
				let _lock = SNIPE_LOCK.lock().await;

				self.snipe_token = api::microsoft::get_java_token(
					self.client
						.as_ref()
						.expect("client was not created when snipe was checked"),
					&api::xbox::Credentials {
						username: &snipe.email,
						password: &snipe.password,
					},
					Some(&CACHE_DIR),
				)
				.await
				.ok();
			}

			if let Some(snipe) = self.snipe.as_ref() {
				if snipe.needed == 0 {
					return self.snipe.as_ref();
				}

				// we have approximately 15 requests every 30 seconds, per account
				// we want to spread these out as much as possible across all workers on the snipe
				//
				// the worker index is the value of `snipe.count - 1`, out of the total `snipe.needed`
				// we want to wait for the closest multiple of the correct time. use the `created_at` time
				// as a base

				let worker_index = snipe.count - 1;
				let worker_count = snipe.needed;

				// the the offset for the current worker in the 2_000ms period
				#[allow(clippy::cast_possible_truncation)]
				let period_offset = (2_000. / f64::from(worker_count) * f64::from(worker_index)).round() as i64;
				let now = chrono::Utc::now().timestamp_millis();

				// get the current 2_000ms period
				let period_shot = now % 2_000;

				// example:
				// period_shot = 1_284
				//
				// offset = 1_000 (two workers)
				// in this case, we should be waiting 1,716ms to reach the offset
				// since it's 716ms to the next period, and 1,000ms for the offset we want
				//
				// offset = 1_500
				// in this case, we should be waiting 216ms to reach the offset
				let wait = if period_offset > period_shot {
					period_offset - period_shot
				} else {
					2_000 - period_shot + period_offset
				};

				if wait > 0 {
					let duration = tokio::time::Duration::from_millis(wait.unsigned_abs());
					tokio::time::sleep(duration).await;
				}
			}

			return self.snipe.as_ref();
		}

		self.snipe = diesel::update(schema::snipe::table)
			.filter(schema::snipe::count.lt(schema::snipe::needed))
			.set((schema::snipe::count.eq(schema::snipe::count + 1),))
			.returning((
				schema::snipe::username,
				schema::snipe::created_at,
				schema::snipe::needed,
				schema::snipe::count,
				schema::snipe::email,
				schema::snipe::password,
			))
			.get_result::<Snipe>(&mut self.pool.get().ok()?)
			.ok();

		self.snipe.as_ref()
	}
}

impl Submit for Postgres {
	async fn submit(
		&self,
		username: &str,
		status: Status,
	) -> Result<(bool, f64), Box<dyn std::error::Error>> {
		if let (Some(snipe), Some(token)) = (self.snipe.as_ref(), self.snipe_token.as_ref()) {
			if snipe.username == username
				&& status == Status::Available
				&& sniper::snipe(username, &token.token).await
			{
				diesel::delete(schema::snipe::table)
					.filter(schema::snipe::username.eq(username))
					.execute(&mut self.pool.get()?)?;

				println!("[{}] Sniped {username}!", time());
			}
		}

		let status: i16 = status.into();
		let conditional_update = sql::<Timestamptz>(&format!(
			"CASE WHEN \"status\" != {status} AND (\"status\" != 5 OR {status} != 2) THEN CURRENT_TIMESTAMP ELSE \"updatedAt\" END",
		))
		.into_sql();

		Ok(diesel::update(schema::name::table)
			.filter(schema::name::username.eq(username))
			.set((
				schema::name::verified_at.eq(diesel::dsl::now),
				schema::name::updated_at.eq(conditional_update),
				schema::name::updating.eq(false),
				schema::name::status.eq(status),
			))
			.returning((
				// CURRENT_TIMESTAMP includes microseconds, but only milliseconds are stored in the database
				// so we need to truncate the timestamp to milliseconds in order to see if the timestamp has changed
				schema::name::updated_at.eq(date_trunc("milliseconds", diesel::dsl::now)),
				schema::name::frequency,
			))
			.get_result::<(bool, f64)>(&mut self.pool.get()?)?)
	}
}

impl HighPrioritySource for Postgres {
	async fn next_high(&mut self) -> Option<String> {
		if let Some(snipe) = self.check_for_snipe().await {
			return Some(snipe.username.clone());
		}

		if self.high.is_empty() {
			let usernames = schema::name::table
				.filter(schema::name::updating.eq(false))
				.filter(schema::name::frequency.ge(15.))
				.order((
					schema::name::verified_at.asc(),
					schema::name::frequency.desc(),
				))
				.limit(100)
				.select(schema::name::username)
				.into_boxed();

			self.high = diesel::update(schema::name::table)
				.filter(schema::name::username.eq_any(usernames))
				.set(schema::name::updating.eq(true))
				.returning(schema::name::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.high.pop()
	}
}

impl MediumPrioritySource for Postgres {
	async fn next_medium(&mut self) -> Option<String> {
		if let Some(snipe) = self.check_for_snipe().await {
			return Some(snipe.username.clone());
		}

		if self.medium.is_empty() {
			let usernames = schema::name::table
				.filter(schema::name::updating.eq(false))
				.filter(
					schema::name::frequency
						.ge(0.01)
						.and(schema::name::frequency.lt(15.)),
				)
				.filter(schema::name::status.ne(i16::from(Status::BatchTaken)))
				.order((
					schema::name::verified_at.asc(),
					schema::name::frequency.desc(),
				))
				.limit(100)
				.select(schema::name::username)
				.into_boxed();

			self.medium = diesel::update(schema::name::table)
				.filter(schema::name::username.eq_any(usernames))
				.set(schema::name::updating.eq(true))
				.returning(schema::name::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.medium.pop()
	}
}

impl LowPrioritySource for Postgres {
	async fn next_low(&mut self) -> Option<String> {
		if let Some(snipe) = self.check_for_snipe().await {
			return Some(snipe.username.clone());
		}

		if self.low.is_empty() {
			let usernames = schema::name::table
				.filter(schema::name::updating.eq(false))
				.filter(schema::name::status.ne(i16::from(Status::BatchTaken)))
				.filter(
					schema::name::frequency.lt(0.01).and(
						schema::name::frequency
							.ge(0.001)
							.or(schema::name::definition.is_not_null()),
					),
				)
				.order((
					schema::name::verified_at.asc(),
					schema::name::frequency.desc(),
				))
				.limit(100)
				.select(schema::name::username)
				.into_boxed();

			self.low = diesel::update(schema::name::table)
				.filter(schema::name::username.eq_any(usernames))
				.set(schema::name::updating.eq(true))
				.returning(schema::name::username)
				.get_results::<String>(&mut self.pool.get().ok()?)
				.ok()?;
		}

		self.low.pop()
	}
}
