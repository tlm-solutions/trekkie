use crate::routes::ServerError;
use crate::DbPool;

use std::env;

use tlms::locations::gps::{GpsPoint, InsertGpsPoint};
use tlms::locations::LocationsJson;
use tlms::management::user::{Role, User};
use tlms::measurements::FinishedMeasurementInterval;
use tlms::telegrams::r09::R09SaveTelegram;
use tlms::trekkie::TrekkieRun;

use lofi::correlate::correlate;

use actix_identity::Identity;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use futures::{StreamExt, TryStreamExt};
use gpx;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// This model is needed after submitting a file the id references the id in the [`SubmitFile`] model.
/// The vehicles are measurements intervals recording start / end and which vehicle in the city was
/// taken
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravel {
    #[schema(example = "
    [ {
        start: Utc::now().naive_utc(),
        end: Utc::now().naive_utc(),
        line: 69,
        run: 42,
        region: 0
    } ]")]
    #[serde(flatten)]
    pub run: FinishedMeasurementInterval,
}

/// This model is returned after uploading a file. It returns the travel id, which is used for
/// submitting the measurement intervals with the [`SubmitTravel`] model
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitRun {
    pub trekkie_run: Uuid,
}

/// Model to correlate runs for given user. If get_result is true, the stops.json also returned
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct CorrelatePlease {
    pub run_id: Uuid,
    pub get_result: bool,
    pub get_stats: bool,
}

/// Response to explicit correlate request
#[derive(Serialize, Deserialize, ToSchema)]
pub struct CorrelateResponse {
    pub new_report_points: i64,
    pub updated_report_points: i64,
    pub stops_file: Option<LocationsJson>,
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
    if corr_request.get_stats {
        return Err(ServerError::NotImplemented);
    }

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

    // fetch the user
    use tlms::schema::users::dsl::users;
    use tlms::schema::users::id as user_id;

    // Get the user and privileges
    let req_user: User = match users
        .filter(user_id.eq(uuid))
        .first(&mut database_connection)
    {
        Ok(user) => user,
        Err(e) => {
            error!("While trying to query user info for {uuid}: {e}");
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

    if run.owner != req_user.id && req_user.role != Role::Administrator as i32 {
        warn!(
            "naughty boy: user {} tried to access run owned by {}!",
            req_user.id, run.owner
        );
        return Err(ServerError::Forbidden);
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

    let queried_gps: Vec<InsertGpsPoint> = queried_gps.into_iter().map(|p| p.into()).collect();

    // query r09 telegrams matching the timeframe of the run
    use tlms::schema::r09_telegrams::dsl::r09_telegrams;
    use tlms::schema::r09_telegrams::line as tg_line;
    use tlms::schema::r09_telegrams::run_number as tg_run;
    use tlms::schema::r09_telegrams::time as telegram_time;
    let telegrams: Vec<R09SaveTelegram> = match r09_telegrams
        .filter(telegram_time.ge(run.start_time))
        .filter(telegram_time.le(run.end_time))
        .filter(tg_line.eq(run.line))
        .filter(tg_run.eq(run.run))
        .load(&mut database_connection)
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

    // get region data cache
    let cache_dir = env::var("TREKKIE_CACHE_DIR").unwrap_or("/tmp/trekkie".to_string());
    let datacare_api =
        env::var("TREKKIE_DATACARE_API").unwrap_or("https://datacare.dvb.solutions".to_string());
    let corr_window: i64 = match env::var("TREKKIE_CORRELATE_WINDOW") {
        Ok(w) => match w.parse() {
            Ok(uwu) => uwu,
            Err(e) => {
                warn!("While trying to parse $TREKKIE_CORRELATE_WINDOW: {e}");
                info!("setting correlation window to default value: 5");
                5
            }
        },
        Err(e) => {
            warn!("While trying to get $TREKKIE_CORRELATE_WINDOW: {e}");
            info!("setting correlation window to default value: 5");
            5
        }
    };
    let reg_cache = match LocationsJson::update_region_cache(&datacare_api, cache_dir.into()) {
        Ok(cache) => cache,
        Err(e) => {
            error!("while trying to get region cache: {e:?}");
            return Err(ServerError::InternalError);
        }
    };

    // corrrelate
    let telegram_iter = Box::new(telegrams.into_iter());
    let stops = correlate(telegram_iter, queried_gps.into(), corr_window, reg_cache);

    // get old stops json
    // TODO: still need to figure out how to store it
    if corr_request.get_result {
        Ok(web::Json(CorrelateResponse {
            new_report_points: -1,
            updated_report_points: -1,
            stops_file: Some(stops),
        }))
    } else {
        Ok(web::Json(CorrelateResponse {
            new_report_points: -1,
            updated_report_points: -1,
            stops_file: None,
        }))
    }
}

/// This endpoint accepts measurement intervals that belong to the previously submitted gpx
/// file.
#[utoipa::path(
    post,
    path = "/travel/submit/run",
    responses(
        (status = 200, description = "travel was successfully submitted", body = crate::routes::SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_submit_run(
    pool: web::Data<DbPool>,
    user: Identity,
    measurement: web::Json<SubmitTravel>,
    _req: HttpRequest,
) -> Result<web::Json<SubmitRun>, ServerError> {
    // getting the database connection from pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    let run_id = Uuid::new_v4();
    match diesel::insert_into(trekkie_runs)
        .values(&TrekkieRun {
            id: run_id,
            start_time: measurement.run.start,
            end_time: measurement.run.stop,
            line: measurement.run.line,
            run: measurement.run.run,
            region: measurement.run.region,
            owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
            finished: true,
        })
        .execute(&mut database_connection)
    {
        Ok(_result) => Ok(web::Json(SubmitRun {
            trekkie_run: run_id,
        })),
        Err(e) => {
            error!("while trying to insert trekkie run {:?}", e);
            Err(ServerError::InternalError)
        }
    }
}

/// Takes the gpx file, saves it, and returns the travel id
#[utoipa::path(
    post,
    path = "/travel/submit/gpx/{}",
    responses(
        (status = 200, description = "gpx file was successfully submitted", body = SubmitFile),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_file_upload(
    pool: web::Data<DbPool>,
    _user: Identity,
    mut payload: Multipart,
    path: web::Path<(Uuid,)>,
    _req: HttpRequest,
) -> Result<HttpResponse, ServerError> {
    // getting the database connection from pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    // collection of gps points
    let mut point_list = Vec::new();

    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let _content_type = field.content_disposition();
        let mut buffer: String = String::new();

        // Merging all the multipart elements into one string
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            let data_string = data.escape_ascii().to_string();

            buffer += &data_string;
        }

        // Deserializing the string into a gpx object
        match gpx::read(buffer.as_bytes()) {
            Ok(gpx) => {
                // I feel like my IQ dropping around here, but dunno how to do it, especially given time
                // situation in gpx crate
                for track in gpx.tracks {
                    for segment in track.segments {
                        for point in segment.points {
                            let soul = InsertGpsPoint {
                                id: None,
                                trekkie_run: path.0,
                                lat: point.point().y(), // according to gpx crate team x and y are less
                                lon: point.point().x(), // ambiguous for coordinates on a map
                                elevation: point.elevation,
                                timestamp: match point.time {
                                    Some(time) => chrono::naive::NaiveDateTime::parse_from_str(
                                        &time.format().unwrap(),
                                        "%Y-%m-%dT%H:%M:%SZ",
                                    )
                                    .unwrap(),
                                    None => break,
                                },

                                accuracy: point.pdop,
                                vertical_accuracy: point.vdop,
                                bearing: None,
                                speed: point.speed,
                            };

                            point_list.push(soul);
                        }
                    }
                }
            }
            Err(e) => {
                error!("cannot convert multipart string into gpx {:?}", e);
                return Err(ServerError::BadClientData);
            }
        }
    }

    use tlms::schema::gps_points::dsl::gps_points;

    // taking all the points and inserting them into the database
    match diesel::insert_into(gps_points)
        .values(&point_list)
        .execute(&mut database_connection)
    {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => {
            error!("while trying to insert trekkie run {:?}", e);
            Err(ServerError::InternalError)
        }
    }
}

/// Takes the gpx file saves it and returns the travel id
#[utoipa::path(
    get,
    path = "/travel/submit/list",
    responses(
        (status = 200, description = "returns old measurements", body = Vec<TrekkieRun>),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_list(
    pool: web::Data<DbPool>,
    user: Identity,
    _req: HttpRequest,
) -> Result<web::Json<Vec<TrekkieRun>>, ServerError> {
    // getting the database connection from pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    use tlms::schema::trekkie_runs::owner;

    match trekkie_runs
        .filter(owner.eq(Uuid::parse_str(&user.id().unwrap()).unwrap()))
        .load::<TrekkieRun>(&mut database_connection)
    {
        Ok(value) => Ok(web::Json(value)),
        Err(e) => {
            error!("was unable runs for user with error {:?}", e);
            Err(ServerError::InternalError)
        }
    }
}
