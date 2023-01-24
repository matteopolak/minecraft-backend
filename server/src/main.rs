mod handlers;

use actix_cors::Cors;
use actix_web::{http::header, web, App, HttpServer};
use diesel::{
	r2d2::{ConnectionManager, Pool},
	PgConnection,
};
use dotenv::dotenv;

pub type PostgresPool = Pool<ConnectionManager<PgConnection>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	dotenv().ok();

	let pool = database::get_pool();

	HttpServer::new(move || {
		let cors = Cors::default()
			.allow_any_origin()
			.allow_any_method()
			.allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
			.send_wildcard();

		App::new()
			.wrap(cors)
			.app_data(web::Data::new(pool.clone()))
			.service(handlers::names::view_names)
			.service(handlers::names::like_name)
			.service(handlers::names::dislike_name)
	})
	.bind(("0.0.0.0", 8080))?
	.run()
	.await
}
