use crate::routes::{user::fetch_user, ServerError};
use crate::DbPool;

use tlms::grpc::GrpcGpsPoint;
use tlms::locations::gps::{GpsPoint, InsertGpsPoint};
use tlms::trekkie::TrekkieRun;

use actix_identity::Identity;
use actix_multipart::Multipart;
use actix_web::{delete, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Duration, TimeZone, Utc};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use futures::{StreamExt, TryStreamExt};
use gpx;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// This struct is send to trekkie to declare a trekkie run
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravelV1 {
    pub start: DateTime<Utc>,
    pub stop: DateTime<Utc>,
    pub line: i32,
    pub run: i32,
    pub region: i64,
}

/// This struct is send to trekkie to declare a trekkie run
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravelV2 {
    pub line: i32,
    pub run: i32,
    pub region: i64,
    pub app_commit: String,
    pub app_name: String,
}

/// GPS Struct
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitGpsPoint {
    pub timestamp: DateTime<Utc>,
    pub lat: f64,
    pub lon: f64,
    pub elevation: Option<f64>,
    pub accuracy: Option<f64>,
    pub vertical_accuracy: Option<f64>,
    pub bearing: Option<f64>,
    pub speed: Option<f64>,
}

/// This model is returned after uploading a file. It returns the travel id, which is used for
/// submitting the measurement intervals with the [`SubmitTravel`] model
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitRun {
    pub trekkie_run: Uuid,
}

/// This endpoint accepts measurement intervals that belong to the previously submitted gpx
/// file.
#[utoipa::path(
    post,
    path = "/v2/trekkie",
    responses(
        (status = 200, description = "travel was successfully submitted", body = SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/trekkie")]
pub async fn travel_submit_run_v1(
    pool: web::Data<DbPool>,
    user: Identity,
    measurement: web::Json<SubmitTravelV1>,
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
            start_time: measurement.start.naive_utc() - Duration::hours(2),
            end_time: measurement.stop.naive_utc() - Duration::hours(2),
            line: measurement.line,
            run: measurement.run,
            region: measurement.region,
            owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
            finished: true,
            correlated: false,
            app_commit: "0000000000000000000000000000000000000000".to_string(),
            app_name: "stasi".to_string(),
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

/// This endpoint accepts measurement intervals that belong to the previously submitted gpx
/// file.
#[utoipa::path(
    post,
    path = "/v2/trekkie",
    responses(
        (status = 200, description = "travel was successfully submitted", body = SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/trekkie")]
pub async fn travel_submit_run_v2(
    pool: web::Data<DbPool>,
    user: Identity,
    measurement: web::Json<SubmitTravelV2>,
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
            start_time: Utc.timestamp_millis_opt(0).unwrap().naive_utc(),
            end_time: Utc.timestamp_millis_opt(0).unwrap().naive_utc(),
            line: measurement.line,
            run: measurement.run,
            region: measurement.region,
            owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
            finished: false,
            correlated: false,
            app_commit: measurement.app_commit.clone(),
            app_name: measurement.app_name.clone(),
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

/// this endpoint takes live gps data from stasi apps
#[utoipa::path(
    delete,
    path = "/v2/trekkie/{id}",
    responses(
        (status = 200, description = "run was successfully terminated",),
        (status = 500, description = "postgres pool error")
    ),
)]
#[delete("/trekkie/{id}")]
pub async fn terminate_run(
    pool: web::Data<DbPool>,
    user: Identity,
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

    let user_session = fetch_user(user, &mut database_connection)?;

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    use tlms::schema::trekkie_runs::finished;
    use tlms::schema::trekkie_runs::id as trekkie_id;

    let this_trekkie_run = match trekkie_runs
        .filter(trekkie_id.eq(path.0))
        .first::<TrekkieRun>(&mut database_connection)
    {
        Ok(found_run) => found_run,
        Err(e) => {
            error!("database error while listing trekkie_runs {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    if !(user_session.is_admin() || user_session.user.id == this_trekkie_run.owner) {
        return Err(ServerError::Forbidden);
    }

    if this_trekkie_run.finished {
        error!(
            "user tried to finish already finished trekkie run {:?}",
            &this_trekkie_run.id
        );
        return Err(ServerError::Conflict);
    }

    use tlms::schema::gps_points::dsl::gps_points;
    use tlms::schema::gps_points::{timestamp, trekkie_run};

    let start_gps = match gps_points
        .filter(trekkie_run.eq(path.0))
        .order(timestamp.asc())
        .limit(1)
        .first::<GpsPoint>(&mut database_connection)
    {
        Ok(value) => value,
        Err(e) => {
            error!("cannot find gps points {:?}", &e);
            return Err(ServerError::InternalError);
        }
    };

    let end_gps = match gps_points
        .filter(trekkie_run.eq(path.0))
        .order(timestamp.desc())
        .limit(1)
        .first::<GpsPoint>(&mut database_connection)
    {
        Ok(value) => value,
        Err(e) => {
            error!("cannot find gps points {:?}", &e);
            return Err(ServerError::InternalError);
        }
    };

    use tlms::schema::trekkie_runs::{end_time, start_time};
    match diesel::update(trekkie_runs)
        .filter(trekkie_id.eq(path.0))
        .set((
            finished.eq(true),
            start_time.eq(start_gps.timestamp),
            end_time.eq(end_gps.timestamp),
        ))
        .execute(&mut database_connection)
    {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => {
            error!("cannot finish this trekkie run with error {:?}", e);
            Err(ServerError::InternalError)
        }
    }
}

/// this endpoint takes live gps data from stasi apps
#[utoipa::path(
    post,
    path = "/v2/trekkie/{id}/live",
    responses(
        (status = 200, description = "travel was successfully submitted", body = SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/trekkie/{id}/live")]
pub async fn submit_gps_live(
    pool: web::Data<DbPool>,
    user: Identity,
    gps_point: web::Json<SubmitGpsPoint>,
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

    let user_session = fetch_user(user, &mut database_connection)?;

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    use tlms::schema::trekkie_runs::id as trekkie_id;

    let trekkie_run = match trekkie_runs
        .filter(trekkie_id.eq(path.0))
        .first::<TrekkieRun>(&mut database_connection)
    {
        Ok(trekkie_run) => trekkie_run,
        Err(e) => {
            error!("database error while listing trekkie_runs {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    if !(user_session.is_admin() || user_session.user.id == trekkie_run.owner) {
        return Err(ServerError::Forbidden);
    }

    if trekkie_run.finished {
        return Err(ServerError::Conflict);
    }

    use tlms::grpc::chemo_client::ChemoClient;

    let grpc_host = match std::env::var("CHEMO_GRPC") {
        Ok(value) => value,
        Err(_e) => {
            error!("NO grpc specified");
            return Err(ServerError::InternalError);
        }
    };

    match ChemoClient::connect(grpc_host.clone()).await {
        Ok(mut client) => {
            let grpc_gps = GrpcGpsPoint {
                time: gps_point.timestamp.timestamp_millis() as u64,
                id: 0,
                region: trekkie_run.region,
                lat: gps_point.lat,
                lon: gps_point.lon,
                line: trekkie_run.line,
                run: trekkie_run.run,
            };

            let request = tonic::Request::new(grpc_gps);
            if let Err(e) = client.receive_gps(request).await {
                warn!("Error while sending gps point: {:?}", e);
            }
        }
        Err(e) => {
            warn!(
                "Cannot connect to GRPC Host: {} with error {:?}",
                grpc_host, &e
            );
        }
    };

    use tlms::schema::gps_points::dsl::gps_points;

    // taking all the points and inserting them into the database
    match diesel::insert_into(gps_points)
        .values(&InsertGpsPoint {
            id: None,
            trekkie_run: path.0,
            timestamp: gps_point.timestamp.naive_utc(),
            lat: gps_point.lat,
            lon: gps_point.lon,
            elevation: gps_point.elevation,
            accuracy: gps_point.accuracy,
            bearing: gps_point.bearing,
            speed: gps_point.speed,
            vertical_accuracy: gps_point.vertical_accuracy,
        })
        .execute(&mut database_connection)
    {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => {
            error!("while trying to insert gps position run {:?}", e);
            Err(ServerError::InternalError)
        }
    }
}

/// Takes the gpx file, saves it, and returns the travel id
#[utoipa::path(
    post,
    path = "/v2/trekkie/{id}/gpx",
    responses(
        (status = 200, description = "gpx file was successfully submitted"),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/trekkie/{id}/gpx")]
pub async fn travel_file_upload(
    pool: web::Data<DbPool>,
    user: Identity,
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

    let user_session = fetch_user(user, &mut database_connection)?;

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    use tlms::schema::trekkie_runs::id as trekkie_id;

    let trekkie_run = match trekkie_runs
        .filter(trekkie_id.eq(path.0))
        .first::<TrekkieRun>(&mut database_connection)
    {
        Ok(trekkie_run) => trekkie_run,
        Err(e) => {
            error!("database error while listing trekkie_runs {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    if !(user_session.is_admin() || user_session.user.id == trekkie_run.owner) {
        return Err(ServerError::Forbidden);
    }

    // collection of gps points
    let mut point_list = Vec::new();

    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let _content_type = field.content_disposition();
        let mut buffer: actix_web::web::BytesMut =
            actix_web::web::BytesMut::with_capacity(1024 * 256);

        // Merging all the multipart elements into one string
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            buffer.extend(data);
        }

        // Deserializing the string into a gpx object
        match gpx::read(buffer.as_ref()) {
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
                                    Some(time) => {
                                        match chrono::naive::NaiveDateTime::parse_from_str(
                                            &time.format().unwrap(),
                                            "%Y-%m-%dT%H:%M:%S.%fZ",
                                        ) {
                                            Ok(result) => result,
                                            Err(e) => {
                                                error!("cannot parse timestamp {e}");
                                                return Err(ServerError::BadClientData);
                                            }
                                        }
                                    }
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
