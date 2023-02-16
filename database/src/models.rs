use chrono::{DateTime, Utc};
use diesel::prelude::Queryable;

#[derive(Queryable)]
pub struct Name {
	pub username: String,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub checked_at: DateTime<Utc>,
	pub verified_at: DateTime<Utc>,
	pub definition: Option<Vec<String>>,
	pub frequency: f32,
	pub length: i32,
	pub updating: bool,
	pub tags: Option<Vec<String>>,
	pub status: i16,
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

#[derive(Queryable)]
pub struct Snipe {
	pub username: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub needed: i16,
	pub count: i16,
	pub email: String,
	pub password: String,
}
