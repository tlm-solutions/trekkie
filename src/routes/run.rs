use crate::routes::ServerError;
use crate::DbPool;

use tlms::locations::gps::InsertGpsPoint;
use tlms::measurements::FinishedMeasurementInterval;
use tlms::trekkie::TrekkieRun;

use actix_identity::Identity;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, post, get};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use futures::{StreamExt, TryStreamExt};
use gpx;
use log::error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};

/// This model is needed after submitting a file the id references the id in the [`SubmitFile`] model.
/// The vehicles are measurements intervals recording start / end and which vehicle in the city was
/// taken
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravelV1 {
    pub start: DateTime<Utc>,
    pub stop: DateTime<Utc>,
    pub line: i32,
    pub run: i32,
    pub region: i64
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravelV2 {
    pub start: DateTime<Utc>,
    pub stop: DateTime<Utc>,
    pub line: i32,
    pub run: i32,
    pub region: i64,
    pub app_commit: String,
    pub app_name: String
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
    path = "/travel/submit/run",
    responses(
        (status = 200, description = "travel was successfully submitted", body = crate::routes::SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/travel/submit/run")]
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
            app_name: "stasi".to_string()
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
    path = "/travel/submit/run",
    responses(
        (status = 200, description = "travel was successfully submitted", body = crate::routes::SubmitRun),
        (status = 500, description = "postgres pool error")
    ),
)]
#[post("/travel/submit/run")]
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
            start_time: measurement.start.naive_utc(),
            end_time: measurement.stop.naive_utc(),
            line: measurement.line,
            run: measurement.run,
            region: measurement.region,
            owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
            finished: true,
            correlated: false,
            app_commit: measurement.app_commit.clone(),
            app_name: measurement.app_name.clone()
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
#[post("/travel/submit/gpx/{id}")]
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
        let mut buffer: actix_web::web::BytesMut =
            actix_web::web::BytesMut::with_capacity(1024 * 256);

        // Merging all the multipart elements into one string
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            buffer.extend(data);
        }

        println!("debug gpx: {:?}", buffer);

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
                                    Some(time) => chrono::naive::NaiveDateTime::parse_from_str(
                                        &time.format().unwrap(),
                                        "%Y-%m-%dT%H:%M:%SZ",
                                    )
                                    .unwrap(), //TODO: fix the unwrap
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
#[get("/travel/submit/list")]
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
