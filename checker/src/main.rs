#![warn(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
mod account;
mod connectors;

use account::Error;
use connectors::prelude::{
	Connector, HighPrioritySource, LowPrioritySource, MediumPrioritySource, Submit,
};
use database::{get_pool, Status};
use once_cell::sync::Lazy;
use reqwest::header;
use serde::Serialize;

static HTTP: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);
static PROXIES_PER_ACCOUNT: usize = 4;

#[derive(Serialize)]
struct PushedPayload<'a> {
	app_key: &'a str,
	app_secret: &'a str,
	target_type: &'a str,
	content: &'a str,
}

fn time() -> String {
	chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("{} Starting...", time());
	dotenvy::dotenv().ok();

	let app_key = std::env::var("APP_KEY").expect("APP_KEY must be set");
	let app_secret = std::env::var("APP_SECRET").expect("APP_SECRET must be set");
	let pool = get_pool();

	// use postgres connector
	let (mut proxies, accounts) = {
		let connector = connectors::sources::postgres::Postgres::new(pool.clone(), None);

		// reset the status of all accounts
		connector.reset()?;

		let proxies = connector.get_proxies()?.into_iter();
		let accounts = connector.get_accounts()?;

		(proxies, accounts)
	};

	let proxies = proxies.by_ref();
	let mut tasks = Vec::new();

	for (index, mut account) in accounts.into_iter().enumerate() {
		for proxy in proxies.take(PROXIES_PER_ACCOUNT) {
			account.add_agent(proxy);
		}

		// mod by 10 to get the last digit
		// digits 0 to 4 are high priority
		// digits 5 to 7 are medium priority
		// digits 8 to 9 are low priority
		let priority = index % 10;
		let priority = match priority {
			0..=4 => 0,
			5..=7 => 1,
			8..=9 => 2,
			_ => unreachable!(),
		};

		// spawn a new tokio task for each account
		tasks.push(tokio::spawn({
			let mut connector = connectors::sources::postgres::Postgres::new(
				pool.clone(),
				account.get_client().cloned(),
			);
			let app_key = app_key.clone();
			let app_secret = app_secret.clone();

			async move {
				'outer: loop {
					while let Some(name) = match priority {
						0 => connector.next_high().await,
						1 => connector.next_medium().await,
						2 => connector.next_low().await,
						_ => unreachable!(),
					} {
						let mut first = true;
						let status = loop {
							let status = account.check(&name, first).await;

							first = false;

							match status {
								Ok(status) => {
									println!(
										"[{}] {} is {} ({})",
										time(),
										name,
										match status {
											Status::Unknown => "unknown",
											Status::Available | Status::BatchAvailable =>
												"available",
											Status::Taken | Status::BatchTaken => "unavailable",
											Status::Banned => "banned",
										},
										match priority {
											0 => "high",
											1 => "medium",
											2 => "low",
											_ => unreachable!(),
										}
									);

									break status;
								}
								Err(Error::Delay(duration)) => {
									println!(
										"[{}] {} is rate limited, waiting {} seconds",
										time(),
										name,
										duration.as_secs()
									);

									tokio::time::sleep(duration).await;

									continue;
								}
								Err(Error::NoClient) => {
									println!("[{}] {} has no clients, exiting", time(), name);

									break 'outer;
								}
								Err(Error::Token) => {
									let seconds = 120;

									println!(
										"[{}] {} could not get token, waiting {} seconds",
										time(),
										name,
										seconds
									);

									tokio::time::sleep(tokio::time::Duration::from_secs(seconds))
										.await;

									continue;
								}
								Err(e) => {
									println!("[{}] {} is invalid: {:?}", time(), name, e);

									continue;
								}
							}
						};

						let is_available = status == Status::Available;
						let (updated, freq) =
							connector.submit(&name, status).await.unwrap_or((false, 0.));

						if updated && is_available && freq > 10. {
							HTTP.post("https://api.pushed.co/1/push")
								.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
								.form(&PushedPayload {
									app_key: &app_key,
									app_secret: &app_secret,
									target_type: "app",
									content: &format!("{name} is now available! ({freq:.2})"),
								})
								.send()
								.await
								.ok();
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

	Ok::<_, Box<dyn std::error::Error>>(())
}
