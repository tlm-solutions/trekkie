use crate::routes::{Response, ServerError};
use crate::DbPool;

use tlms::management::user::{hash_password, verify_password, AuthorizedUser, User};

use log::{error, info};
use uuid::Uuid;

use actix_identity::Identity;
use actix_web::{post, web, HttpMessage, HttpRequest};
use diesel::{ExpressionMethods, PgConnection, QueryDsl, RunQueryDsl};
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Response body if user creation is successful. User ID and password are returned.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserCreation {
    pub user_id: Uuid,
    pub password: String,
}

/// Request body for authentication
#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserLogin {
    pub user_id: Uuid,
    pub password: String,
}

/// takes a cookie and returnes the corresponging user struct
pub fn fetch_user(
    user: Identity,
    database_connection: &mut PgConnection,
) -> Result<AuthorizedUser, ServerError> {
    // user uuid from currently authenticat
    let user_id: Uuid = match user.id() {
        Ok(found_id) => match Uuid::parse_str(&found_id) {
            Ok(parsed_uuid) => parsed_uuid,
            Err(e) => {
                error!("problem with decoding id from cookie {:?}", e);
                return Err(ServerError::BadClientData);
            }
        },
        Err(e) => {
            error!("problem with fetching id from cookie {:?}", e);
            return Err(ServerError::BadClientData);
        }
    };

    AuthorizedUser::from_postgres(&user_id, database_connection).ok_or(ServerError::BadClientData)
}

/// Request to this endpoint creates minimal and unpriviledged trekkie user. If the call was succesful
/// user information and a session cookie are returned
#[utoipa::path(
    post,
    path = "/v2/user",
    responses(
        (status = 200, description = "trekkie user was successfully created", body = UserCreation),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/user")]
pub async fn user_create(
    pool: web::Data<DbPool>,
    req: HttpRequest,
) -> Result<web::Json<UserCreation>, ServerError> {
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };
    let user_id = Uuid::new_v4();
    let password: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let hashed_password = match hash_password(&password) {
        Some(data) => data,
        None => {
            error!("cannot hash user password");
            return Err(ServerError::BadClientData);
        }
    };

    use tlms::schema::users::dsl::users;
    if let Err(e) = diesel::insert_into(users)
        .values(&User {
            id: user_id,
            name: None,
            email: None,
            password: hashed_password,
            deactivated: false,
            email_setting: None,
            admin: false,
        })
        .execute(&mut database_connection)
    {
        error!("while trying to insert trekkie user {:?}", e);
        return Err(ServerError::BadClientData);
    };
    info!("creating new user with id {}", user_id);

    if let Err(e) = Identity::login(&req.extensions(), user_id.to_string()) {
        error!("Cannot create session! {e:?}");
        error!("Is redis running?");
        return Err(ServerError::BadClientData);
    };

    Ok(web::Json(UserCreation { user_id, password }))
}

/// Sends user credentials to the server. If they are correct a session cookie is set.
#[utoipa::path(
    post,
    path = "/v2/auth/login",
    request_body = UserLogin,
    responses(
        (status = 200, description = "trekkie user was successfully logged in", body = Response),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/auth/login")]
pub async fn user_login(
    pool: web::Data<DbPool>,
    body: web::Json<UserLogin>,
    req: HttpRequest,
) -> Result<web::Json<Response>, ServerError> {
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    info!("user with id {} has logged in", &body.user_id);

    use tlms::schema::users::dsl::users;
    use tlms::schema::users::id;
    let user = match users
        .filter(id.eq(body.user_id))
        .first::<User>(&mut database_connection)
    {
        Ok(data) => data,
        Err(e) => {
            error!("Err: {:?}", e);
            return Err(ServerError::BadClientData);
        }
    };

    if verify_password(&body.password, &user.password) {
        match Identity::login(&req.extensions(), user.id.to_string()) {
            Ok(_) => {}
            Err(e) => {
                error!(
                    "cannot create session maybe the redis is not running. {:?}",
                    e
                );
                return Err(ServerError::BadClientData);
            }
        };

        Ok(web::Json(Response { success: true }))
    } else {
        Ok(web::Json(Response { success: false }))
    }
}
