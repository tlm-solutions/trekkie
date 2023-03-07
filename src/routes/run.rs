use crate::routes::ServerError;
use crate::DbPool;

use tlms::locations::gps::InsertGpsPoint;
use tlms::measurements::FinishedMeasurementInterval;
use tlms::trekkie::TrekkieRun;

use futures::{StreamExt, TryStreamExt};
use gpx;
use log::error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use actix_identity::Identity;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

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
    path = "/travel/submit/gpx",
    responses(
        (status = 200, description = "gpx file was successfully submitted", body = SubmitFile),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_file_upload(
    pool: web::Data<DbPool>,
    _user: Identity,
    run: web::Json<SubmitRun>,
    mut payload: Multipart,
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
                                trekkie_run: run.trekkie_run,
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
