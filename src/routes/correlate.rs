use std::collections::HashMap;

use crate::routes::ServerError;
use crate::utils::get_authorized_user;
use crate::DbPool;

use diesel::upsert::on_constraint;
use lofi::correlate::correlate_trekkie_run;
use tlms::locations::gps::GpsPoint;
use tlms::locations::{
    InsertTransmissionLocation, InsertTransmissionLocationRaw, TransmissionLocationRaw,
};
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

/// Request to update all transmission locations
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct UpdateAllLocationsResponse {
    /// amount of upserted positions
    rows_affected: usize,
}

/// This endpoint takes all the transmission_locaions_raw, and dedupes them into the transmission
/// locations. If location exists, updates it, if not: inserts it. Needless to say: this is
/// extremely expensive endpoint, so requires admin privelege.
#[utoipa::path(
    post,
    path = "/locations/update_all",
    responses(
        (status = 200, description = "Correlation Successful", body = CorrelateAllResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Interal Error"),
        (status = 501, description = "Not Implemented"),
    ),
)]
pub async fn update_all_transmission_locations(
    pool: web::Data<DbPool>,
    user: Identity,
    _req: HttpRequest,
) -> Result<web::Json<UpdateAllLocationsResponse>, ServerError> {
    // get connection from the pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    // Get the user and privileges
    let req_user = get_authorized_user(user, &pool)?;

    if !req_user.is_admin() {
        error!(
            "User {usr} is not admin, and requested to update all positions!",
            usr = req_user.user.id
        );
        return Err(ServerError::Forbidden);
    }

    // Load the raw runs wholesale
    use tlms::schema::r09_transmission_locations_raw::dsl::r09_transmission_locations_raw;
    let raw_locs: Vec<TransmissionLocationRaw> =
        match r09_transmission_locations_raw.load(&mut database_connection) {
            Ok(l) => l,
            Err(e) => {
                error!("while trying to fetch r09_transmission_locations_raw: {e}");
                return Err(ServerError::InternalError);
            }
        };

    // group the locations by region/location
    let mut raw_loc_groups: HashMap<(i64, i32), Vec<TransmissionLocationRaw>> = HashMap::new();
    for i in raw_locs {
        raw_loc_groups
            .entry((i.region, i.reporting_point))
            .or_insert(Vec::new())
            .push(i);
    }

    // convert raw locations to deduped ones
    let ins_deduped_locs: Vec<InsertTransmissionLocation> = raw_loc_groups
        .iter()
        .map(|(_k, v)| InsertTransmissionLocation::try_from_raw(v.clone()))
        .filter_map(|res| {
            res.map_err(|_e| {
                error!("Error while deduping raw locations into production ones!");
            })
            .ok()
        })
        .collect();

    // upsert the deduped locations
    use diesel::pg::upsert::excluded;
    use tlms::schema::r09_transmission_locations::dsl::r09_transmission_locations;
    use tlms::schema::r09_transmission_locations::lat;
    use tlms::schema::r09_transmission_locations::lon;
    let rows_affected = match diesel::insert_into(r09_transmission_locations)
        .values(&ins_deduped_locs)
        .on_conflict(on_constraint(
            tlms::locations::REGION_POSITION_UNIQUE_CONSTRAINT,
        ))
        .do_update()
        .set((lat.eq(excluded(lat)), lon.eq(excluded(lon))))
        .execute(&mut database_connection)
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("While trying to upsert into r09_transmission_locations: {e}");
            return Err(ServerError::InternalError);
        }
    };

    Ok(web::Json(UpdateAllLocationsResponse { rows_affected }))
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

    // Get the user and privileges
    let req_user = get_authorized_user(user, &pool)?;

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
