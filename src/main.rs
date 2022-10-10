mod structs;
mod routes;

use structs::Args;

use clap::Parser;
use log::{info, debug};

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use diesel::r2d2::Pool;

use actix_web::{
    web, App, HttpServer, 
    cookie::Key
};

use actix_identity::IdentityMiddleware;
use actix_session::SessionMiddleware;
use actix_session::storage::RedisActorSessionStore;

use utoipa::OpenApi;
//use utoipa_swagger_ui::{SwaggerUi, Url};

use std::env;
use std::fmt::format;
use std::fs;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub fn create_db_pool() -> DbPool {
    let default_postgres_host = String::from("localhost:5433");
    let default_postgres_port = String::from("5432");
    let default_postgres_pw_path = String::from("/run/secrets/postgres_password");

    let password_path = env::var("POSTGRES_PASSWORD_PATH")
        .unwrap_or(default_postgres_pw_path.clone());
    let password = fs::read_to_string(password_path).expect("cannot read password file!");

    let database_url = format!(
        "postgres://dvbdump:{}@{}:{}/dvbdump",
        password,
        env::var("POSTGRES_HOST").unwrap_or(default_postgres_host.clone()),
        env::var("POSTGRES_PORT").unwrap_or(default_postgres_port.clone())
    );

    debug!("Connecting to postgres database {}", &database_url);
    let manager = ConnectionManager::<PgConnection>::new(database_url);

    Pool::new(manager).expect("Failed to create pool.")
}

pub fn get_redis_uri() -> String {
    let default_redis_port = "6379".to_string();
    let default_redis_host = "127.0.0.1".to_string();

    format!("{}:{}",
        std::env::var("REDIS_HOST").unwrap_or(default_redis_host),
        std::env::var("REDIS_PORT").unwrap_or(default_redis_port)
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args = Args::parse();

    if args.swagger {
        println!("{}", routes::ApiDoc::openapi().to_pretty_json().unwrap());
        return Ok(());
    }

    info!("Starting Data Collection Server ... ");
    let host = args.api_host.as_str();
    let port = args.port;
    debug!("Listening on: {}:{}", host, port);

    let connection_pool = web::Data::new(create_db_pool());
    let secret_key = Key::generate();
    HttpServer::new( move || {
        App::new()
            .wrap(IdentityMiddleware::default())
            .wrap(SessionMiddleware::new(
                 RedisActorSessionStore::new(get_redis_uri()),
                 secret_key.clone()
            ))
            .app_data(connection_pool.clone())
            .service(web::resource("/travel/submit/gpx").route(web::post().to(routes::run::travel_file_upload)))
            .route("/travel/submit/run", web::post().to(routes::run::travel_submit_run))
            .route("/user/create", web::post().to(routes::user::user_create))
            .route("/user/login", web::post().to(routes::user::user_login))
            /*.service(SwaggerUi::new("/swagger-ui/{_:.*}").urls(vec![
                (
                    Url::new("api", "/api-doc/openapi.json"),
                    routes::ApiDoc::openapi(),
                ),
            ])) */
    })
    .bind((host, port))?
    .run()
    .await
}
