
use crate::routes::{Response, ServerError};
use crate::DbPool;

use dump_dvb::measurements::FinishedMeasurementInterval;

use dump_dvb::trekkie::InsertTrekkieRun;

use utoipa::ToSchema;
use serde::{Serialize, Deserialize};
use log::error;
use uuid::Uuid;
use futures::{TryStreamExt, StreamExt};

use std::fs::File;
use std::io::Write;

use actix_identity::Identity;
use actix_web::{web, HttpRequest};
use actix_multipart::Multipart;
use diesel::RunQueryDsl;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmittTravel {
    pub gpx_id: Uuid,
    pub vehicles: Vec<FinishedMeasurementInterval>
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SubmittedFile {
    pub success: bool,
    pub gpx_id: Uuid 
}


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
    submission: web::Json<SubmittTravel>,
    _req: HttpRequest,
    ) ->  Result<web::Json<Response>, ServerError> {
    let mut database_connection = match pool.get() {
         Ok(conn) => conn,
         Err(e) => {
             error!("cannot get connection from connection pool {:?}", e);
             return Err(ServerError::InternalError);
         }
    };

    // todo: authenticate

    use dump_dvb::schema::trekkie_runs::dsl::trekkie_runs;
    for measurement in (*submission).vehicles.clone().into_iter() {
        match diesel::insert_into(trekkie_runs)
            .values(&InsertTrekkieRun {
            id: None,
            start_time: measurement.start,
            end_time: measurement.stop,
            line: measurement.line,
            run: measurement.run,
            gps_file: submission.gpx_id.to_string(),
            region: measurement.region as i64,
            owner: Uuid::parse_str(&user.id().unwrap()).unwrap(),
            finished: true
        })
        .execute(&mut database_connection) {
            Err(e) => {
                error!("while trying to insert trekkie run {:?}", e);
            }
            _ => {}
        };
    }

    return Ok(web::Json(Response { success: false }))
}



#[utoipa::path(
    post,
    path = "/travel/submit/gpx",
    responses(
        (status = 200, description = "gpx file was successfully submitted", body = crate::routes::SubmittedFile),
        (status = 500, description = "postgres pool error")
    ),
)]
pub async fn travel_file_upload(
    // pool: web::Data<DbPool>,
    _user: Identity,
    mut payload: Multipart,
    _req: HttpRequest,
) ->  Result<web::Json<SubmittedFile>, ServerError> {
    let default_gpx_path = "/var/lib/trekkie/gpx/".to_string();
    let gpx_path = std::env::var("GPX_PATH").unwrap_or(default_gpx_path);
    let run_uuid = Uuid::new_v4();
    let filepath = format!("{}{}.gpx", gpx_path, &run_uuid);

    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let _content_type = field.content_disposition();
        let filepath_clone = filepath.clone();

        // File::create is blocking operation, use threadpool
        let mut f: File = match web::block(|| std::fs::File::create(filepath_clone))
            .await {
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

    Ok(web::Json(SubmittedFile { success: true , gpx_id: run_uuid }))
}

