pub async fn snipe(username: &str, token: &str) -> bool {
	let client = reqwest::Client::new();

	let response = client
		.put(format!(
			"https://api.minecraftservices.com/minecraft/profile/name/{username}"
		))
		.header(reqwest::header::AUTHORIZATION, token)
		.send()
		.await;

	match response {
		Ok(response) => {
			if response.status() == reqwest::StatusCode::OK {
				println!("{username} has been sniped!");

				true
			} else {
				println!(
					"{username} could not be sniped! (status: {})",
					response.status()
				);

				false
			}
		}
		Err(e) => {
			println!("{username} could not be sniped: {e}");

			false
		}
	}
}
