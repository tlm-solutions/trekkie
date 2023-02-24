use crate::routes::{Response, ServerError};
use crate::DbPool;

use tlms::measurements::FinishedMeasurementInterval;
use tlms::trekkie::{InsertTrekkieRun, TrekkieRun};

use futures::{StreamExt, TryStreamExt};
use log::error;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use std::fs::File;
use std::io::Write;

use actix_identity::Identity;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

/// This model is needed after submitting a file the id references the id in the SubmitFile model.
/// The vehicles are measurements intervals recording start / end and which vehicle in the city was
/// taken
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitTravel {
    pub gpx_id: Uuid,

    #[schema(example = "
    [ {
        start: Utc::now().naive_utc(),
        end: Utc::now().naive_utc(),
        line: 69,
        run: 42,
        region: 0
    } ]")]
    pub vehicles: Vec<FinishedMeasurementInterval>,
}

/// This model is returned after uploading a file it returnes the travel id which is used for
/// submitting the measurements intervals with the SubmitTravel model
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmitFile {
    pub gpx_id: Uuid,
}

/// This endpoints if submitting measurement intervals that belong to the previous submitted gpx
/// file.
#[utoipa::path(
    post,
    path = "/travel/submit/run",
    responses(
        (status = 200, description = "travel was successfully submitted", body = crate::routes::Response),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_submit_run(
    pool: web::Data<DbPool>,
    user: Identity,
    submission: web::Json<SubmitTravel>,
    _req: HttpRequest,
) -> Result<web::Json<Response>, ServerError> {
    // getting the database connection from pool
    let mut database_connection = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("cannot get connection from connection pool {:?}", e);
            return Err(ServerError::InternalError);
        }
    };

    use tlms::schema::trekkie_runs::dsl::trekkie_runs;
    for measurement in submission.vehicles.clone().into_iter() {
        if let Err(e) = diesel::insert_into(trekkie_runs)
            .values(&InsertTrekkieRun {
                id: None,
                start_time: measurement.start,
                end_time: measurement.stop,
                line: measurement.line,
                run: measurement.run,
                gps_file: submission.gpx_id.to_string(),
                region: measurement.region,
                owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
                finished: true,
            })
            .execute(&mut database_connection)
        {
            error!("while trying to insert trekkie run {:?}", e);
            return Err(ServerError::InternalError);
        };
    }

    Ok(web::Json(Response { success: false }))
}

/// Takes the gpx file saves it and returnes the travel id
#[utoipa::path(
    post,
    path = "/travel/submit/gpx",
    responses(
        (status = 200, description = "gpx file was successfully submitted", body = crate::routes::SubmitFile),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_file_upload(
    // pool: web::Data<DbPool>,
    _user: Identity,
    mut payload: Multipart,
    _req: HttpRequest,
) -> Result<web::Json<SubmitFile>, ServerError> {
    let default_gpx_path = "/var/lib/trekkie/gpx/".to_string();
    let gpx_path = std::env::var("GPX_PATH").unwrap_or(default_gpx_path);
    let run_uuid = Uuid::new_v4();
    let filepath = format!("{}{}.gpx", gpx_path, &run_uuid);

    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let _content_type = field.content_disposition();
        let filepath_clone = filepath.clone();

        // File::create is blocking operation, use threadpool
        let mut f: File = match web::block(|| std::fs::File::create(filepath_clone)).await {
            Ok(wrapped_file) => match wrapped_file {
                Ok(file) => file,
                Err(e) => {
                    error!("cannot create uploaded file because of file error {:?}", e);
                    return Err(ServerError::InternalError);
                }
            },
            Err(e) => {
                error!("cannot create uploaded file because of blockerror {:?}", e);
                return Err(ServerError::InternalError);
            }
        };

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = chunk.unwrap();
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f))
                .await
                .unwrap()
                .unwrap();
        }
    }

    Ok(web::Json(SubmitFile { gpx_id: run_uuid }))
}

/// Takes the gpx file saves it and returnes the travel id
#[utoipa::path(
    get,
    path = "/travel/submit/list",
    responses(
        (status = 200, description = "returnes old measurements", body = Vec<TrekkieRun>),
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
