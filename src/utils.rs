use crate::DbPool;
use crate::routes::ServerError;

use uuid::Uuid;
use log::error;

use tlms::management::user::AuthorizedUser;

pub fn get_authorized_user(user: actix_identity::Identity, pool: actix_web::web::Data<DbPool>) -> Result<AuthorizedUser, ServerError> {
    // get connection from the pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    // Parse the user id
    let uuid: Uuid = match user.id() {
        Ok(id) => match Uuid::parse_str(&id) {
            Ok(uid) => uid,
            Err(e) => {
                error!("While parsing user UUID: {e}");
                return Err(ServerError::Unauthorized);
            }
        },
        Err(e) => {
            error!("While trying to read user id from request: {e}");
            return Err(ServerError::Unauthorized);
        }
    };

    // Get the user and privileges
        match AuthorizedUser::from_postgres(&uuid, &mut database_connection) {
            Some(user) => Ok(user),
            None => Err(ServerError::Unauthorized),
        }
}

