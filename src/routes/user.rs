use crate::routes::{Response, ServerError};
use crate::DbPool;

use dump_dvb::management::{user::{Role, User, hash_password, verify_password}};

use log::{error, info};
use uuid::Uuid;

use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpMessage};
use diesel::{RunQueryDsl, QueryDsl, ExpressionMethods};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Serialize, Deserialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserCreation {
    pub success: bool,
    pub user_id: Uuid,
    pub password: String
}


#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserLogin{
    pub user_id: Uuid,
    pub password: String
}

#[utoipa::path(
    post,
    path = "/user/create",
    responses(
        (status = 200, description = "trekkie user was successfully created", body = crate::routes::UserCreation),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn user_create(
    pool: web::Data<DbPool>,
    req: HttpRequest,
    ) ->  Result<web::Json<UserCreation>, ServerError> {
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
    
    info!("creating new user with id {}", user_id);

    use dump_dvb::schema::users::dsl::users;
    match diesel::insert_into(users)
        .values(&User {
        id: user_id,
        name: None,
        email: None,
        password: hashed_password,
        role: Role::Trekkie.as_int(),
        deactivated: false,
        email_setting: None
    })
    .execute(&mut database_connection) {
        Err(e) => {
            error!("while trying to insert trekkie user {:?}", e);
        }
        _ => {}
    };

    Identity::login(&req.extensions(), user_id.to_string().into()).unwrap();

    Ok(web::Json(UserCreation { 
        success: true,
        user_id,
        password
    }))
}


#[utoipa::path(
    post,
    path = "/user/login",
    responses(
        (status = 200, description = "trekkie user was successfully logged in", body = crate::routes::Response),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn user_login(
    pool: web::Data<DbPool>,
    body: web::Json<UserLogin>,
    req: HttpRequest,
    ) ->  Result<web::Json<Response>, ServerError> {
    let mut database_connection = match pool.get() {
         Ok(conn) => conn,
         Err(e) => {
             error!("cannot get connection from connection pool {:?}", e);
             return Err(ServerError::InternalError);
         }
    };

    info!("user with id {} has logged in", &body.user_id);

    use dump_dvb::schema::users::dsl::users;
    use dump_dvb::schema::users::id;
    let user = match users 
        .filter(id.eq(body.user_id))
        .first::<User>(&mut database_connection) {
        Ok(data) => {
            data
        }
        Err(e) => {
            error!("Err: {:?}", e);
            return Err(ServerError::BadClientData);
        }
    };

    if verify_password(&body.password, &user.password) {
        Identity::login(&req.extensions(), user.id.to_string().into()).unwrap();

        Ok(web::Json(Response { success: true }))
    } else {
        Ok(web::Json(Response { success: false }))
    }
}
