use actix_web::{get, http::header, post, web, HttpRequest, HttpResponse};
use database::{schema, PostgresPool};
use diesel::prelude::*;
use diesel::{
	ExpressionMethods, JoinOnDsl, NullableExpressionMethods, PgConnection, QueryDsl, Queryable,
	RunQueryDsl, TextExpressionMethods,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ViewNamesOptions {
	pub limit: Option<i64>,
	pub offset: Option<i64>,
	pub sort: Option<String>,
	pub column: Option<String>,
	pub search: Option<String>,
	pub tags: Option<Vec<String>>,
	pub from: Option<chrono::NaiveDateTime>,
	pub to: Option<chrono::NaiveDateTime>,
}

#[derive(Queryable, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedName {
	pub username: String,
	pub frequency: f64,
	pub definition: Option<Vec<String>>,
	pub tags: Option<Vec<String>>,
	pub verified_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub valid: Option<bool>,
	pub liked: Option<bool>,
}

#[derive(Serialize)]
pub struct ViewNamesResponse {
	pub data: Vec<FormattedName>,
	pub total: i64,
}

#[derive(Serialize)]
pub struct NameResponse {
	pub updated: bool,
}

#[post("/names")]
pub async fn view_names(
	data: web::Json<ViewNamesOptions>,
	req: HttpRequest,
	pool: web::Data<PostgresPool>,
) -> Result<HttpResponse, actix_web::Error> {
	// get the token from the "Authorization" header, or return a 401 if it does not exist or cannot be parsed
	let token = req
		.headers()
		.get(header::AUTHORIZATION)
		.ok_or(actix_web::error::ErrorUnauthorized(""))?
		.to_str()
		.map_err(|_| actix_web::error::ErrorUnauthorized(""))?;

	// get the user from the database, or return a 401 if the token is invalid
	let user_id = schema::users::table
		.select(schema::users::id)
		.filter(schema::users::key.eq(token))
		.get_result::<i32>(
			&mut pool
				.get()
				.map_err(|_| actix_web::error::ErrorInternalServerError(""))?,
		);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	let query = || {
		let mut names = schema::names::table.into_boxed().left_join(
			schema::likes::table.on(schema::likes::username
				.eq(schema::names::username)
				.and(schema::likes::user_id.eq(user_id))),
		);

		if let Some(search) = &data.search {
			names = names
				.filter(schema::names::username.like(format!("%{}%", search.to_ascii_lowercase())))
		}

		if data
			.tags
			.as_ref()
			.map(|tags| tags.contains(&"new".to_string()))
			.unwrap_or(false)
		{
			// filter for updated_at to be within the last 24 hours
			names = names.filter(
				schema::names::updated_at
					.ge(chrono::Utc::now().naive_utc() - chrono::Duration::days(1)),
			);
		} else if let Some(from) = data.from {
			names = names.filter(schema::names::updated_at.ge(from));
		}

		if let Some(to) = data.to {
			names = names.filter(schema::names::updated_at.le(to));
		}

		// 'common' is a tag, filter for frequency >= 0.5
		// 'short' is a tag, filter for length <= 7
		// 'liked' is a tag, filter for liked = true
		// 'taken' is a tag, filter for valid = false
		// 'name' is a tag, filter for 'name' IN tags

		let mut other_tags = Vec::new();
		let mut has_taken_tag = false;

		if let Some(tags) = data.tags.as_ref() {
			for tag in tags {
				match tag.as_str() {
					"common" => names = names.filter(schema::names::frequency.ge(0.5)),
					"short" => names = names.filter(schema::names::length.le(7)),
					"liked" => {
						names = names.filter(schema::likes::username.is_not_null());
					}
					"taken" => {
						names = names.filter(schema::names::valid.is_not_null());
						has_taken_tag = true;
					}
					"name" => names = names.filter(schema::names::tags.contains(vec!["name"])),
					tag => other_tags.push(tag),
				}
			}
		}

		if !has_taken_tag {
			names = names.filter(schema::names::valid.eq(true));
		}

		if !other_tags.is_empty() {
			names = names.filter(schema::names::tags.contains(other_tags));
		}

		names
	};

	let mut names = query()
		.limit(data.limit.unwrap_or(10))
		.offset(data.offset.unwrap_or(0))
		.select((
			schema::names::username,
			schema::names::frequency,
			schema::names::definition.nullable(),
			schema::names::tags.nullable(),
			schema::names::verified_at,
			schema::names::updated_at,
			schema::names::valid
				.nullable()
				.or(false.into_sql::<diesel::sql_types::Bool>()),
			// TRUE if it is liked, FALSE otherwise -- default to FALSE if the value is NULL
			schema::likes::username
				.is_not_null()
				.nullable()
				.or(false.into_sql::<diesel::sql_types::Bool>()),
		));

	let names = {
		match (data.sort.as_deref(), data.column.as_deref()) {
			(Some("asc"), Some("frequency")) => {
				names = names.order(schema::names::frequency.asc());
			}
			(Some("asc"), Some("length")) => {
				names = names.order((schema::names::length.asc(), schema::names::frequency.desc()))
			}
			(_, Some("length")) => {
				names = names.order((
					schema::names::length.desc(),
					schema::names::frequency.desc(),
				))
			}
			(Some("asc"), Some("updatedAt")) => {
				names = names.order((
					schema::names::updated_at.asc(),
					schema::names::frequency.desc(),
				))
			}
			(_, Some("updatedAt")) => {
				names = names.order((
					schema::names::updated_at.desc(),
					schema::names::frequency.desc(),
				))
			}
			(Some("asc"), Some("verifiedAt")) => {
				names = names.order((
					schema::names::verified_at.asc(),
					schema::names::frequency.desc(),
				))
			}
			(_, Some("verifiedAt")) => {
				names = names.order((
					schema::names::verified_at.desc(),
					schema::names::frequency.desc(),
				))
			}
			(Some("asc"), Some("username")) => {
				names = names.order((
					schema::names::username.asc(),
					schema::names::frequency.desc(),
				))
			}
			(_, Some("username")) => {
				names = names.order((
					schema::names::username.desc(),
					schema::names::frequency.desc(),
				))
			}
			_ => names = names.order(schema::names::frequency.desc()),
		}

		names
	};

	let names = names
		.load::<FormattedName>(
			&mut pool
				.get()
				.map_err(|_| actix_web::error::ErrorInternalServerError(""))?,
		)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	let count = query()
		.count()
		.get_result::<i64>(
			&mut pool
				.get()
				.map_err(|_| actix_web::error::ErrorInternalServerError(""))?,
		)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(ViewNamesResponse {
		data: names,
		total: count,
	}))
}

#[get("/names/{name}/like")]
pub async fn like_name(
	name: web::Path<String>,
	req: HttpRequest,
	pool: web::Data<PostgresPool>,
) -> Result<HttpResponse, actix_web::Error> {
	let token = req
		.headers()
		.get(header::AUTHORIZATION)
		.ok_or(actix_web::error::ErrorUnauthorized(""))?
		.to_str()
		.map_err(|_| actix_web::error::ErrorUnauthorized(""))?;

	let connection = &mut pool
		.get()
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	let connection: &mut PgConnection = connection;

	let user_id = schema::users::table
		.select(schema::users::id)
		.filter(schema::users::key.eq(token))
		.get_result::<i32>(connection);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	// insert into likes (username, user_id) values ($1, $2) on conflict do nothing
	// get the user_id from the token in the same query

	let result = diesel::insert_into(schema::likes::table)
		.values((
			schema::likes::username.eq(name.into_inner()),
			schema::likes::user_id.eq(user_id),
		))
		.on_conflict_do_nothing()
		.execute(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(NameResponse {
		updated: result > 0,
	}))
}

#[get("/names/{name}/dislike")]
pub async fn dislike_name(
	name: web::Path<String>,
	req: HttpRequest,
	pool: web::Data<PostgresPool>,
) -> Result<HttpResponse, actix_web::Error> {
	let token = req
		.headers()
		.get(header::AUTHORIZATION)
		.ok_or(actix_web::error::ErrorUnauthorized(""))?
		.to_str()
		.map_err(|_| actix_web::error::ErrorUnauthorized(""))?;

	let connection = &mut pool
		.get()
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	let connection: &mut PgConnection = connection;

	let user_id = schema::users::table
		.select(schema::users::id)
		.filter(schema::users::key.eq(token))
		.get_result::<i32>(connection);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	let result = diesel::delete(schema::likes::table)
		.filter(
			schema::likes::username
				.eq(name.into_inner())
				.and(schema::likes::user_id.eq(user_id)),
		)
		.execute(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(NameResponse {
		updated: result > 0,
	}))
}
