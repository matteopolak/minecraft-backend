use diesel::prelude::Queryable;

#[derive(Queryable)]
pub struct Name {
	pub username: String,
	pub popularity: f64,
	pub available: bool,
	pub created_at: chrono::NaiveDateTime,
	pub updated_at: chrono::NaiveDateTime,
	pub checked_at: chrono::NaiveDateTime,
	pub valid: Option<bool>,
	pub verified_at: chrono::NaiveDateTime,
	pub definition: Option<Vec<String>>,
	pub frequency: f32,
	pub length: i32,
	pub updating: bool,
	pub tags: Vec<String>,
}

#[derive(Queryable)]
pub struct User {
	pub id: i32,
	pub key: String,
}

#[derive(Queryable)]
pub struct Proxy {
	pub id: i32,
	pub address: String,
	pub port: i32,
	pub username: Option<String>,
	pub password: Option<String>,
	pub note: Option<String>,
}

#[derive(Queryable)]
pub struct Account {
	pub id: i32,
	pub username: String,
	pub password: String,
}

#[derive(Queryable)]
pub struct Like {
	pub username: String,
	pub user_id: i32,
}
