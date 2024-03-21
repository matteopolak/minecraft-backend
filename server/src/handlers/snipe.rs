use actix_web::{http::header, post, web, HttpRequest, HttpResponse};
use database::{schema, PostgresPool};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateSnipeOptions {
	pub username: String,
	pub email: String,
	pub password: String,
	pub workers: i16,
}

#[derive(Serialize)]
pub struct CreateSnipeResponse {
	pub updated: bool,
}

#[post("/snipe")]
pub async fn create_snipe(
	data: web::Json<CreateSnipeOptions>,
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

	let _user_id = schema::user::table
		.select(schema::user::id)
		.filter(schema::user::key.eq(token))
		.get_result::<i32>(connection)
		.map_err(|_| actix_web::error::ErrorUnauthorized(""))?;

	// add the snipe to the database
	let updates = diesel::insert_into(schema::snipe::table)
		.values((
			schema::snipe::username.eq(&data.username),
			schema::snipe::needed.eq(&data.workers),
			schema::snipe::email.eq(&data.email),
			schema::snipe::password.eq(&data.password),
		))
		.on_conflict_do_nothing()
		.execute(connection)
		.map_err(|_| actix_web::error::ErrorInternalServerError(""))?;

	Ok(HttpResponse::Ok().json(CreateSnipeResponse {
		updated: updates > 0,
	}))
}
