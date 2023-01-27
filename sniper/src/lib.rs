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
			if response.status() == reqwest::StatusCode::NO_CONTENT {
				println!("{username} has been sniped!");
			} else {
				println!("{username} could not be sniped!");
			}

			response.status() != reqwest::StatusCode::FORBIDDEN
		}
		Err(e) => {
			println!("{username} could not be sniped: {e}");

			false
		}
	}
}
