mod routes;
mod structs;

use structs::Args;

use actix_identity::IdentityMiddleware;
use actix_session::storage::RedisActorSessionStore;
use actix_session::SessionMiddleware;
use actix_web::{cookie::Key, middleware::Logger, web, App, HttpServer};
use clap::Parser;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::Pool;
use diesel::PgConnection;
use log::{debug, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use std::env;
use std::fs;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn create_db_pool() -> DbPool {
    let default_postgres_host = String::from("localhost:5433");
    let default_postgres_port = String::from("5432");
    let default_postgres_database = String::from("tlms");

    let password_path =
        env::var("TREKKIE_POSTGRES_PASSWORD_PATH").expect("DB password was not specified");
    let password = fs::read_to_string(password_path).expect("cannot read password file!");
    let postgres_user = env::var("TREKKIE_POSTGRES_USER").expect("no database user configured");

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        postgres_user,
        password,
        env::var("TREKKIE_POSTGRES_HOST").unwrap_or(default_postgres_host),
        env::var("TREKKIE_POSTGRES_PORT").unwrap_or(default_postgres_port),
        env::var("TREKKIE_POSTGRES_DATABAE").unwrap_or(default_postgres_database)
    );

    debug!("Connecting to postgres database {}", &database_url);
    let manager = ConnectionManager::<PgConnection>::new(database_url);

    Pool::new(manager).expect("Failed to create pool.")
}

pub fn get_redis_uri() -> String {
    let default_redis_port = "6379".to_string();
    let default_redis_host = "127.0.0.1".to_string();

    format!(
        "{}:{}",
        std::env::var("TREKKIE_REDIS_HOST").unwrap_or(default_redis_host),
        std::env::var("TREKKIE_REDIS_PORT").unwrap_or(default_redis_port)
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.swagger {
        println!(
            "{}",
            routes::ApiDoc::openapi()
                .to_pretty_json()
                .expect("could not format openapi spec!")
        );
        return Ok(());
    }

    env_logger::init();

    info!("Starting Data Collection Server ... ");
    let host = args.api_host.as_str();
    let port = args.port;
    info!("Listening on: {}:{}", host, port);

    let connection_pool = web::Data::new(create_db_pool());
    let secret_key = Key::generate();

    HttpServer::new(move || {
        App::new()
            .wrap(IdentityMiddleware::default())
            .wrap(SessionMiddleware::new(
                RedisActorSessionStore::new(get_redis_uri()),
                secret_key.clone(),
            ))
            .wrap(Logger::default())
            .app_data(connection_pool.clone())
            .service(
                web::scope("/v1")
                    .service(routes::run::travel_file_upload)
                    .service(routes::run::travel_submit_run_v1)
                    .service(routes::user::user_create)
                    .service(routes::user::user_login)

            )
            .service(
                web::scope("/v2")
                    .service(routes::run::travel_file_upload)
                    .service(routes::run::travel_submit_run_v2)
                    .service(routes::run::submit_gps_live)
                    .service(routes::run::terminate_run)
                    .service(routes::user::user_create)
                    .service(routes::user::user_login)

            )
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-doc/openapi.json", routes::ApiDoc::openapi()),
            )
    })
    .bind((host, port))?
    .run()
    .await
}
