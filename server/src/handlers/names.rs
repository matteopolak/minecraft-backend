use actix_web::{get, http::header, post, web, HttpRequest, HttpResponse};
use database::{schema, PostgresPool, Status};
use diesel::prelude::*;
use diesel::Queryable;
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
	pub status: i16,
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

	let connection = &mut pool
		.get()
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	// get the user from the database, or return a 401 if the token is invalid
	let user_id = schema::user::table
		.select(schema::user::id)
		.filter(schema::user::key.eq(token))
		.get_result::<i32>(connection);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	let query = || {
		let mut names = schema::name::table.into_boxed().left_join(
			schema::like::table.on(schema::like::username
				.eq(schema::name::username)
				.and(schema::like::user_id.eq(user_id))),
		);

		if let Some(search) = &data.search {
			names = names
				.filter(schema::name::username.like(format!("%{}%", search.to_ascii_lowercase())))
		}

		if data
			.tags
			.as_ref()
			.map(|tags| tags.contains(&"new".to_string()))
			.unwrap_or(false)
		{
			// filter for updated_at to be within the last 24 hours
			names = names.filter(schema::name::updated_at.ge(chrono::Utc::now().naive_utc()
				- chrono::Duration::try_days(1).expect("1 day to be less than i64::MAX / 1_000")));
		} else if let Some(from) = data.from {
			names = names.filter(schema::name::updated_at.ge(from));
		}

		if let Some(to) = data.to {
			names = names.filter(schema::name::updated_at.le(to));
		}

		let mut other_tags = Vec::new();
		let mut has_taken_tag = false;

		if let Some(tags) = data.tags.as_ref() {
			for tag in tags {
				match tag.as_str() {
					"common" => names = names.filter(schema::name::frequency.ge(0.5)),
					"short" => names = names.filter(schema::name::length.le(7)),
					"liked" => {
						names = names.filter(schema::like::username.is_not_null());
					}
					"taken" => {
						names = names
							.filter(schema::name::status.ne(i16::from(Status::BatchAvailable)));
						has_taken_tag = true;
					}
					"banned" => {
						names = names.filter(schema::name::status.eq(i16::from(Status::Banned)));
						has_taken_tag = true;
					}
					"name" => names = names.filter(schema::name::tags.contains(vec!["name"])),
					tag => other_tags.push(tag),
				}
			}
		}

		if !has_taken_tag {
			names = names.filter(schema::name::status.eq(i16::from(Status::Available)));
		}

		if !other_tags.is_empty() {
			names = names.filter(schema::name::tags.contains(other_tags));
		}

		names
	};

	let mut names = query()
		.limit(data.limit.unwrap_or(10))
		.offset(data.offset.unwrap_or(0))
		.select((
			schema::name::username,
			schema::name::frequency,
			schema::name::definition.nullable(),
			schema::name::tags.nullable(),
			schema::name::verified_at,
			schema::name::updated_at,
			schema::name::status,
			// TRUE if it is liked, FALSE otherwise -- default to FALSE if the value is NULL
			schema::like::username
				.is_not_null()
				.nullable()
				.or(false.into_sql::<diesel::sql_types::Bool>()),
		));

	let names = {
		match (data.sort.as_deref(), data.column.as_deref()) {
			(Some("asc"), Some("frequency")) => {
				names = names.order(schema::name::frequency.asc());
			}
			(Some("asc"), Some("length")) => {
				names = names.order((schema::name::length.asc(), schema::name::frequency.desc()))
			}
			(_, Some("length")) => {
				names = names.order((schema::name::length.desc(), schema::name::frequency.desc()))
			}
			(Some("asc"), Some("updatedAt")) => {
				names = names.order((
					schema::name::updated_at.asc(),
					schema::name::frequency.desc(),
				))
			}
			(_, Some("updatedAt")) => {
				names = names.order((
					schema::name::updated_at.desc(),
					schema::name::frequency.desc(),
				))
			}
			(Some("asc"), Some("verifiedAt")) => {
				names = names.order((
					schema::name::verified_at.asc(),
					schema::name::frequency.desc(),
				))
			}
			(_, Some("verifiedAt")) => {
				names = names.order((
					schema::name::verified_at.desc(),
					schema::name::frequency.desc(),
				))
			}
			(Some("asc"), Some("username")) => {
				names = names.order((schema::name::username.asc(), schema::name::frequency.desc()))
			}
			(_, Some("username")) => {
				names = names.order((
					schema::name::username.desc(),
					schema::name::frequency.desc(),
				))
			}
			_ => names = names.order(schema::name::frequency.desc()),
		}

		names
	};

	let names = names
		.load::<FormattedName>(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	let count = query()
		.count()
		.get_result::<i64>(connection)
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

	let user_id = schema::user::table
		.select(schema::user::id)
		.filter(schema::user::key.eq(token))
		.get_result::<i32>(connection);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	// insert into likes (username, user_id) values ($1, $2) on conflict do nothing
	// get the user_id from the token in the same query

	let updates = diesel::insert_into(schema::like::table)
		.values((
			schema::like::username.eq(name.into_inner()),
			schema::like::user_id.eq(user_id),
		))
		.on_conflict_do_nothing()
		.execute(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(NameResponse {
		updated: updates > 0,
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

	let user_id = schema::user::table
		.select(schema::user::id)
		.filter(schema::user::key.eq(token))
		.get_result::<i32>(connection);

	let user_id = match user_id {
		Ok(user_id) => user_id,
		Err(_) => return Err(actix_web::error::ErrorUnauthorized("")),
	};

	let updates = diesel::delete(schema::like::table)
		.filter(
			schema::like::username
				.eq(name.into_inner())
				.and(schema::like::user_id.eq(user_id)),
		)
		.execute(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(NameResponse {
		updated: updates > 0,
	}))
}
