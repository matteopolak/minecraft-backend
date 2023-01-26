use database::{schema, PostgresPool, Status};
use diesel::{dsl::sql, sql_types::SmallInt, ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::connectors::prelude::Connector;

pub struct Postgres {
	pool: PostgresPool,
}

impl Postgres {
	pub fn new(pool: PostgresPool) -> Self {
		Self { pool }
	}
}

impl Connector for Postgres {
	fn next(&mut self, size: i64) -> Option<Vec<String>> {
		schema::names::table
			.select(schema::names::username)
			.order(schema::names::checked_at.asc())
			.limit(size)
			.load::<String>(&mut self.pool.get().unwrap())
			.ok()
	}

	fn submit_available(&self, names: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
		// diesel does not support `CASE WHEN` statements, so we can
		// create an sql fragment and use it directly in the query
		//
		// since "available" names may not actually be available, we
		// use the special Status::BatchAvailable status to show that it could
		// be available. if the status is already set to Status::Banned, then
		// it would show up as available in this case, so we don't want to overwrite it.
		// furthermore, if it is already Status::Available, we don't want to overwrite it
		// as it is actually available.
		//
		// Status::Available = 1
		// Status::Banned = 3
		//
		// this query reads like the following:
		// "if the status is Status::Available or Status::Banned, then don't change it, otherwise set it to Status::BatchAvailable"
		let case = sql::<SmallInt>("CASE WHEN \"status\" IN (1, 3) THEN \"status\" ELSE 4 END");

		diesel::update(schema::names::table)
			.filter(schema::names::username.eq_any(&names))
			.set((
				schema::names::checked_at.eq(diesel::dsl::now),
				schema::names::status.eq(case),
			))
			.execute(&mut database::get_pool().get()?)?;

		Ok(())
	}

	fn submit_unavailable(&self, names: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
		// set unavailable names to Status::Taken
		diesel::update(schema::names::table)
			.filter(schema::names::username.eq_any(&names))
			.set((
				schema::names::checked_at.eq(diesel::dsl::now),
				schema::names::status.eq(i16::from(Status::BatchTaken)),
			))
			.execute(&mut database::get_pool().get()?)?;

		Ok(())
	}
}
