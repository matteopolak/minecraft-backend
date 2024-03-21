#![feature(extract_if)]
use futures::StreamExt;
use reqwest::header::{self, HeaderMap};
use serde::Deserialize;

use crate::connectors::prelude::Connector;
mod connectors;

#[derive(Deserialize)]
struct Profile {
	name: String,
}

#[derive(Deserialize)]
struct Response {
	data: Vec<Profile>,
	retry: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	dotenv::dotenv().ok();

	let pool = database::get_pool();
	let mut connector = connectors::sources::postgres::Postgres::new(pool);

	let client = {
		let mut headers = HeaderMap::new();

		headers.insert(
			header::CONTENT_TYPE,
			"application/json"
				.parse()
				.expect("could not serialize content type header"),
		);
		headers.insert(
			"Token",
			std::env::var("SECRET")
				.expect("environment variable SECRET not found")
				.parse()
				.expect("failed to parse SECRET"),
		);

		reqwest::Client::builder()
			.default_headers(headers)
			.build()
			.expect("could not build http client")
	};

	let mut start = std::time::Instant::now();

	while let Some(mut batch) = connector.next(1_000) {
		let result = futures::stream::iter(batch.chunks(250).map(|chunk| {
			let client = client.clone();

			async move {
				let response = client
					.post("https://worker.minecraft.matteopolak.com")
					.json(&chunk)
					.send()
					.await
					.ok()?
					.json::<Response>()
					.await
					.ok()?;

				Some((
					response
						.data
						.into_iter()
						.map(|profile| profile.name)
						.collect::<Vec<_>>(),
					response.retry,
				))
			}
		}))
		.buffer_unordered(10)
		.filter_map(|x| async { x })
		.collect::<Vec<_>>()
		.await;

		let mut pause = false;
		let retry = result
			.iter()
			.flat_map(|(_, retry)| {
				if retry.len() > 100 {
					pause = true;
				}

				retry
			})
			.collect::<std::collections::HashSet<_>>();

		// these names could have capitalization, so we need to lowercase them
		// `to_ascii_lowercase` is used because it's faster than `to_lowercase`
		// and all characters are in the range of ASCII (a-zA-Z0-9_)
		let taken = result
			.iter()
			.flat_map(|(taken, _)| taken.iter().map(|name| name.to_ascii_lowercase()))
			.collect::<std::collections::HashSet<_>>();

		// remove names that were not checked from the availability pool
		batch.retain(|username| !retry.contains(username));

		// mutate the original `batch` vector and remove names that were taken
		// resulting in `batch` being a vector of names that are available
		let unavailable = batch
			.drain_if(|username| taken.contains(username))
			.collect::<Vec<_>>();

		// `batch` now contains names that are *not taken*, which
		// is not the same as being available, as they could be banned or locked.
		let available = batch;

		println!(
			"available: {}, unavailable: {}, skipped: {}",
			available.len(),
			unavailable.len(),
			retry.len()
		);

		if !available.is_empty() {
			connector.submit_available(available)?;
		}

		if !unavailable.is_empty() {
			connector.submit_unavailable(unavailable)?;
		}

		// if more than 100 names were not checked, pause for 5 minutes
		// otherwise, pause for 4 seconds since the last check
		if pause {
			println!("pausing for 5 minutes");
			tokio::time::sleep(std::time::Duration::from_secs(300)).await;
		} else {
			// get the number of milliseconds elapsed since the last check
			// and subtract it from 4 seconds (4000 milliseconds) to get
			// the number of milliseconds to sleep
			if let Some(sleep) =
				// this will panic if the number of milliseconds is greater than u64::MAX
				// i.e. ~300 million years
				4_000u64.checked_sub(start.elapsed().as_millis().try_into().expect(
						"wow, the time elapsed in milliseconds is greater than u64::MAX",
					)) {
				println!("pausing for {sleep} ms");
				tokio::time::sleep(std::time::Duration::from_millis(sleep)).await;
			}
		}

		start = std::time::Instant::now();
	}

	Ok(())
}
