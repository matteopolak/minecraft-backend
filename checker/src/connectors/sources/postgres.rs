use crate::{account::Account, connectors::prelude::*};

const HIGH_PRIORITY_QUERY: &str = r#"
	UPDATE "names" SET "updating" = TRUE
		WHERE "username" IN (
			SELECT "username" FROM "names"
				WHERE "updating" = FALSE AND "frequency" >= 15
				ORDER BY "verifiedAt" ASC, "frequency" DESC
				LIMIT 100
				FOR UPDATE
		)
	RETURNING "username"
"#;

const MEDIUM_PRIORITY_QUERY: &str = r#"
	UPDATE "names" SET "updating" = TRUE
		WHERE "username" IN (
			SELECT "username" FROM "names"
				WHERE "available" = TRUE AND "updating" = FALSE AND "frequency" >= 0.01 AND "frequency" < 20
				ORDER BY "verifiedAt" ASC, "frequency" DESC
				LIMIT 100
				FOR UPDATE
		)
	RETURNING "username"
"#;

const LOW_PRIORITY_QUERY: &str = r#"
	UPDATE "names" SET "updating" = TRUE
		WHERE "username" IN (
			SELECT "username" FROM "names"
				WHERE "available" = TRUE AND "updating" = FALSE AND ("frequency" >= 0.001 OR array_length(definition, 1) > 0) AND "frequency" < 0.01
				ORDER BY "verifiedAt" ASC, "frequency" DESC
				LIMIT 100
				FOR UPDATE
		)
	RETURNING "username"
"#;

pub struct Postgres {
	high: Vec<String>,
	medium: Vec<String>,
	low: Vec<String>,
	client: tokio_postgres::Client,
}

impl Connector for Postgres {
	async fn prepare() -> Result<Self, Box<dyn std::error::Error>>
	where
		Self: Sized,
	{
		let (client, connection) = tokio_postgres::connect(
			&std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable not found"),
			tokio_postgres::NoTls,
		)
		.await?;

		tokio::spawn(async move {
			if let Err(e) = connection.await {
				eprintln!("connection error: {e}");
			}
		});

		Ok(Self {
			high: Vec::new(),
			medium: Vec::new(),
			low: Vec::new(),
			client,
		})
	}

	async fn get_accounts(&self) -> Result<Vec<Account>, Box<dyn std::error::Error>> {
		let rows = self
			.client
			.query(
				r#"
				SELECT username, password
					FROM accounts
					ORDER BY username
			"#,
				&[],
			)
			.await?;

		Ok(rows
			.into_iter()
			.map(|row| {
				// we can leak these strings because they will live for the duration of the program
				Account::new(row.get::<_, String>(0), row.get::<_, String>(1))
			})
			.collect())
	}

	async fn get_proxies(&self) -> Result<Vec<reqwest::Proxy>, Box<dyn std::error::Error>> {
		let rows = self
			.client
			.query(
				r#"
				SELECT address, port, username, password
					FROM proxies
					ORDER BY address
			"#,
				&[],
			)
			.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				// we can leak these strings because they will live for the duration of the program
				Some(
					reqwest::Proxy::https(format!(
						"{}:{}",
						/* host */ row.get::<_, String>(0),
						/* port */ row.get::<_, i32>(1)
					))
					.ok()?
					.basic_auth(
						/* username */ row.get(2),
						/* password */ row.get(3),
					),
				)
			})
			.collect())
	}
}

impl Submit for Postgres {
	async fn submit(
		&self,
		username: &str,
		available: bool,
	) -> Result<(bool, f64), Box<dyn std::error::Error>> {
		let row = self
			.client
			.query_one(
				if available {
					r#"
						UPDATE "names" SET
							"verifiedAt" = NOW(),
							"updatedAt" = (CASE WHEN ("valid" IS NULL OR "valid" = FALSE) THEN NOW() ELSE "updatedAt" END),
							"valid" = TRUE,
							"updating" = FALSE
						WHERE username = $1
						RETURNING "frequency", "valid" IS NULL OR "valid" = FALSE AS "changed"
					"#
				} else {
					r#"
						UPDATE "names" SET
							"verifiedAt" = NOW(),
							"updatedAt" = (CASE WHEN ("valid" IS NULL OR "valid" = TRUE) THEN NOW() ELSE "updatedAt" END),
							"valid" = FALSE,
							"available" = FALSE,
							"updating" = FALSE
						WHERE username = $1
						RETURNING "frequency", FALSE AS "changed"
					"#
				},
				&[&username],
			)
			.await?;

		Ok((row.get(1), row.get(0)))
	}
}

impl HighPrioritySource for Postgres {
	async fn next_high(&mut self) -> Option<String> {
		if self.high.is_empty() {
			self.high = self
				.client
				.query(HIGH_PRIORITY_QUERY, &[])
				.await
				.ok()?
				.into_iter()
				.map(|row| row.get::<_, String>(0))
				.collect();
		}

		self.high.pop()
	}
}

impl MediumPrioritySource for Postgres {
	async fn next_medium(&mut self) -> Option<String> {
		if self.medium.is_empty() {
			self.medium = self
				.client
				.query(MEDIUM_PRIORITY_QUERY, &[])
				.await
				.ok()?
				.into_iter()
				.map(|row| row.get::<_, String>(0))
				.collect();
		}

		self.medium.pop()
	}
}

impl LowPrioritySource for Postgres {
	async fn next_low(&mut self) -> Option<String> {
		if self.low.is_empty() {
			self.low = self
				.client
				.query(LOW_PRIORITY_QUERY, &[])
				.await
				.ok()?
				.into_iter()
				.map(|row| row.get::<_, String>(0))
				.collect();
		}

		self.low.pop()
	}
}
