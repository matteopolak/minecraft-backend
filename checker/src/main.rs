#![feature(string_leak)]
mod account;

use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Arc;
use tokio_postgres::{Error, NoTls};

const HTTP: Lazy<reqwest::Client> = Lazy::new(|| reqwest::Client::new());
const PROXIES_PER_ACCOUNT: usize = 4;
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

#[derive(Serialize)]
struct PushedPayload<'a> {
	app_key: &'static str,
	app_secret: &'static str,
	target_type: &'static str,
	content: &'a str,
}

fn time() -> String {
	chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	dotenv::dotenv().ok();

	let (client, connection) = tokio_postgres::connect(
		&std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable not found"),
		NoTls,
	)
	.await?;

	tokio::spawn(async move {
		if let Err(e) = connection.await {
			eprintln!("connection error: {}", e);
		}
	});

	let mut accounts = client
		.query("SELECT username, password FROM accounts", &[])
		.await
		.unwrap()
		.into_iter();

	let mut proxies = client
		.query(
			"SELECT address, port, username, password FROM proxies ORDER BY address, port",
			&[],
		)
		.await
		.unwrap()
		.into_iter();

	let client_arc = Arc::new(client);

	let mut index = 0;
	let mut tasks = Vec::new();

	while let Some(row) = accounts.next() {
		let username: String = row.get(0);
		let password: String = row.get(1);

		let proxies = proxies.by_ref().take(PROXIES_PER_ACCOUNT).map(|row| {
			reqwest::Proxy::https(&format!(
				"{}:{}",
				/* host */ row.get::<_, String>(0),
				/* port */ row.get::<_, i32>(1)
			))
			.unwrap()
			.basic_auth(
				/* username */ row.get(2),
				/* password */ row.get(3),
			)
		});

		let mut account = account::Account::new(username.leak(), password.leak());
		let mut first = true;

		for proxy in proxies {
			if first {
				first = false;

				println!("first: {:?}", proxy);
			}

			account.add_agent(proxy);
		}

		// mod by 10 to get the last digit
		// digits 0 to 4 are high priority
		// digits 5 to 7 are medium priority
		// digits 8 to 9 are low priority
		let priority = index % 10;
		let query = match priority {
			0..=4 => HIGH_PRIORITY_QUERY,
			5..=7 => MEDIUM_PRIORITY_QUERY,
			8..=9 => LOW_PRIORITY_QUERY,
			_ => unreachable!(),
		};

		index += 1;

		// spawn a new tokio task for each account
		tasks.push(tokio::spawn({
			let client = Arc::clone(&client_arc);

			async move {
				'outer: loop {
					let names = client
						.query(query, &[])
						.await
						.unwrap()
						.into_iter()
						.map(|row| row.get::<_, String>(0));

					println!("[{}] {} names fetched", time(), names.len());

					for name in names {
						match loop {
							let result = account.check(&name).await;

							match result {
								Ok(true) => {
									println!("[{}] {} is available", time(), name);

									break true;
								}
								Ok(false) => {
									println!("[{}] {} is taken", time(), name);

									break false;
								}
								Err(account::Error::Delay(d)) => {
									println!(
										"[{}] {} is rate limited, waiting {} seconds",
										time(),
										name,
										d.as_secs()
									);

									tokio::time::sleep(d).await;

									continue;
								}
								Err(account::Error::NoClientError) => {
									println!("[{}] {} has no clients, exiting", time(), name);

									break 'outer;
								}
								Err(e) => {
									println!("[{}] {} is invalid: {:?}", time(), name, e);

									continue;
								}
							}
						} {
							true => {
								let row = client
									.query_one(
										r#"
											UPDATE "names" SET
												"verifiedAt" = NOW(),
												"updatedAt" = (CASE WHEN ("valid" IS NULL OR "valid" = FALSE) THEN NOW() ELSE "updatedAt" END),
												"valid" = TRUE,
												"updating" = FALSE
											WHERE username = $1
											RETURNING "username", "frequency", "valid" IS NULL OR "valid" = FALSE AS "changed"
										"#,
										&[&name],
									)
									.await
									.unwrap();

								if row.get::<_, bool>(2) {
									let username: String = row.get(0);
									let frequency: f64 = row.get(1);

									// send a notification with pushed.co
									HTTP.post("https://api.pushed.co/1/push")
										.json(&PushedPayload {
											app_key: "7ZbySgthX7JnmlPe3LHv",
											app_secret: "D6sVv0aFEKg479IVI1JcdDaet1GOmc3dPQDWc5jiMFErx88gxjBBl6rtfJ1c8gsA",
											target_type: "app",
											content: &format!(
												"{} is now available! ({:.2})",
												username, frequency
											),
										})
										.send()
										.await
										.ok();
								}
							}
							false => {
								client
									.execute(
										r#"
											UPDATE "names" SET
												"verifiedAt" = NOW(),
												"updatedAt" = (CASE WHEN ("valid" IS NULL OR "valid" = TRUE) THEN NOW() ELSE "updatedAt" END),
												"valid" = FALSE,
												"updating" = FALSE
											WHERE username = $1
										"#,
										&[&name],
									)
									.await
									.unwrap();
							}
						}
					}
				}
			}
		}));
	}

	// wait for all tasks to finish
	// (this should never happen)
	for task in tasks {
		task.await.unwrap();
	}

	Ok(())
}
