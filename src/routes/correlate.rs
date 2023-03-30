use crate::routes::ServerError;
use crate::DbPool;

use lofi::correlate::correlate_trekkie_run;
use tlms::locations::gps::GpsPoint;
use tlms::management::user::AuthorizedUser;
use tlms::telegrams::r09::R09SaveTelegram;
use tlms::trekkie::TrekkieRun;

use actix_identity::Identity;
use actix_web::{web, HttpRequest};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Model to correlate runs for given user. If get_result is true, the stops.json also returned
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct CorrelatePlease {
    /// ID of run to correlate
    pub run_id: Uuid,
    /// Optional value for the corr_window
    pub corr_window: Option<i64>,
}

/// Response to explicit correlate request
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct CorrelateResponse {
    pub success: bool,
    pub new_raw_transmission_locations: i64,
}

/// This endpoint would correlate runs for given user id. For regular user only own runs
/// can be correlated, for admin - any run for any user
#[utoipa::path(
    post,
    path = "/run/correlate",
    responses(
        (status = 200, description = "Correlation Successful", body = CorrelateResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Interal Error"),
        (status = 501, description = "Not Implemented"),
    ),
)]
pub async fn correlate_run(
    pool: web::Data<DbPool>,
    user: Identity,
    _req: HttpRequest,
    corr_request: web::Json<CorrelatePlease>,
) -> Result<web::Json<CorrelateResponse>, ServerError> {
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
    let req_user: AuthorizedUser =
        match AuthorizedUser::from_postgres(&uuid, &mut database_connection) {
            Some(user) => user,
            None => {
                return Err(ServerError::Unauthorized);
            }
        };

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    use tlms::schema::trekkie_runs::id as run_id;
    let run: TrekkieRun = match trekkie_runs
        .filter(run_id.eq(corr_request.run_id))
        .first(&mut database_connection)
    {
        Ok(r) => r,
        Err(e) => {
            error!("While trying to query for run {}: {e}", corr_request.run_id);
            return Err(ServerError::InternalError);
        }
    };

    if run.owner != req_user.user.id && !req_user.is_admin() {
        warn!(
            "naughty boy: user {} tried to access run owned by {}!",
            req_user.user.id, run.owner
        );
        return Err(ServerError::Forbidden);
    }

    if run.correlated {
        info!(
            "User {usr} requested to correlate trekkie run {r}, which is already correlated.",
            usr = req_user.user.id,
            r = run.id
        );
        warn!("Run already correlated. Correlation step skipped.");

        return Ok(web::Json(CorrelateResponse {
            success: true,
            new_raw_transmission_locations: 0,
        }));
    }

    use tlms::schema::gps_points::dsl::gps_points;
    use tlms::schema::gps_points::trekkie_run;
    let queried_gps: Vec<GpsPoint> = match gps_points
        .filter(trekkie_run.eq(run.id))
        .load(&mut database_connection)
    {
        Ok(points) => points,
        Err(e) => {
            error!(
                "while fetching gps points for run id {id}: {e}",
                id = run.id
            );
            return Err(ServerError::InternalError);
        }
    };

    // query r09 telegrams matching the timeframe of the run
    use tlms::schema::r09_telegrams::dsl::r09_telegrams;

    use tlms::schema::r09_telegrams::line as telegram_line;
    use tlms::schema::r09_telegrams::run_number as telegram_run;
    use tlms::schema::r09_telegrams::time as telegram_time;
    let telegrams: Vec<R09SaveTelegram> = match r09_telegrams
        .filter(telegram_time.ge(run.start_time))
        .filter(telegram_time.le(run.end_time))
        .filter(telegram_line.eq(run.line))
        .filter(telegram_run.eq(run.run))
        .load::<R09SaveTelegram>(&mut database_connection)
    {
        Ok(t) => t,
        Err(e) => {
            error!(
                "While trying to query the telegrams matching {run}: {e}",
                run = run.id
            );
            return Err(ServerError::InternalError);
        }
    };

    let corr_window = match corr_request.corr_window {
        Some(x) => x,
        None => lofi::correlate::DEFAULT_CORRELATION_WINDOW,
    };

    // corrrelate
    let locs = match correlate_trekkie_run(&telegrams, queried_gps, corr_window, run.id, run.owner)
    {
        Ok(l) => l,
        Err(_) => {
            return Err(ServerError::InternalError);
        }
    };

    // Insert raw transmission locations into the DB
    use tlms::schema::r09_transmission_locations_raw::dsl::r09_transmission_locations_raw;
    let updated_rows = match diesel::insert_into(r09_transmission_locations_raw)
        .values(&locs)
        .execute(&mut database_connection)
    {
        Ok(r) => r,
        Err(_) => return Err(ServerError::InternalError),
    };

    // Update correlated flag in the trekkie_runs db
    use tlms::schema::trekkie_runs::correlated as trekkie_corr_flag;
    match diesel::update(trekkie_runs)
        .filter(run_id.eq(corr_request.run_id))
        .set(trekkie_corr_flag.eq(true))
        .execute(&mut database_connection)
    {
        Ok(ok) => ok,
        Err(e) => {
            error!("while trying to set `correlated` flag in trekkie_runs: {e:?}");
            return Err(ServerError::InternalError);
        }
    };

    Ok(web::Json(CorrelateResponse {
        success: true,
        new_raw_transmission_locations: updated_rows as i64,
    }))
}
