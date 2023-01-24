#![warn(clippy::pedantic)]
#![feature(async_fn_in_trait)]
#![feature(string_leak)]
#![allow(clippy::too_many_lines)]
mod account;
mod connectors;

use account::Error;
use connectors::prelude::{
	Connector, HighPrioritySource, LowPrioritySource, MediumPrioritySource, Submit,
};
use once_cell::sync::Lazy;
use reqwest::header;
use serde::Serialize;

static HTTP: Lazy<reqwest::Client> = Lazy::new(reqwest::Client::new);
static PROXIES_PER_ACCOUNT: usize = 4;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	dotenv::dotenv().ok();

	// use postgres connector
	let (mut proxies, accounts) = {
		let connector = connectors::sources::postgres::Postgres::prepare().await?;
		let proxies = connector.get_proxies().await?.into_iter();
		let accounts = connector.get_accounts().await?;

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
			let mut connector = connectors::sources::postgres::Postgres::prepare().await?;
			let mut account = account.clone();

			async move {
				'outer: loop {
					while let Some(name) = match priority {
						0 => connector.next_high().await,
						1 => connector.next_medium().await,
						2 => connector.next_low().await,
						_ => unreachable!(),
					} {
						let available = loop {
							let result = account.check(&name).await;

							match result {
								Ok(available) => {
									println!(
										"[{}] {} is {}",
										time(),
										name,
										if available {
											"available"
										} else {
											"unavailable"
										}
									);

									break available;
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

						let (updated, freq) = connector
							.submit(&name, available)
							.await
							.unwrap_or((false, 0.));

						if updated && available {
							HTTP.post("https://api.pushed.co/1/push")
								.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
								.form(&PushedPayload {
									app_key: "7ZbySgthX7JnmlPe3LHv",
									app_secret: "D6sVv0aFEKg479IVI1JcdDaet1GOmc3dPQDWc5jiMFErx88gxjBBl6rtfJ1c8gsA",
									target_type: "app",
									content: &format!(
										"{name} is now available! ({freq:.2})"
									),
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
