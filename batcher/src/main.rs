#![feature(extract_if)]
use std::collections::HashSet;

use futures::StreamExt;

use crate::connectors::prelude::Connector;
mod connectors;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	dotenvy::dotenv().ok();

	let pool = database::get_pool();
	let mut connector = connectors::sources::postgres::Postgres::new(pool);

	let client = reqwest::Client::new();

	let mut start = std::time::Instant::now();

	while let Some(mut batch) = connector.next(1_000) {
		let mut result = futures::stream::iter(batch.iter().map(|chunk| {
			let client = client.clone();

			async move {
				let response = client
					.head(format!("https://mc-heads.net/head/{chunk}"))
					.send()
					.await
					.ok()?;

				if response.headers().contains_key("etag") {
					return Some((Some(chunk.to_string()), None));
				}

				if response.status().is_success() {
					return None;
				}

				Some((None, Some(chunk.to_string())))
			}
		}))
		.buffer_unordered(25)
		.filter_map(|x| async { x })
		.collect::<Vec<_>>()
		.await;

		let retry = result
			.iter()
			.filter_map(|r| r.1.clone())
			.collect::<HashSet<_>>();
		let pause = retry.len() > 100;

		// these names could have capitalization, so we need to lowercase them
		// `to_ascii_lowercase` is used because it's faster than `to_lowercase`
		// and all characters are in the range of ASCII (a-zA-Z0-9_)
		let taken = result
			.iter_mut()
			.flat_map(|(taken, _)| {
				taken.iter_mut().map(|name| {
					name.make_ascii_lowercase();
					&*name
				})
			})
			.collect::<std::collections::HashSet<_>>();

		// remove names that were not checked from the availability pool
		batch.retain(|username| !retry.contains(username));

		// mutate the original `batch` vector and remove names that were taken
		// resulting in `batch` being a vector of names that are available
		let unavailable = batch
			.extract_if(|username| taken.contains(username))
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
				2_000u64.checked_sub(start.elapsed().as_millis().try_into().expect(
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
