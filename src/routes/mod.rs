pub mod run;
pub mod user;

use actix_web::{
    error,
    http::{header::ContentType, StatusCode},
    HttpResponse,
};
use derive_more::{Display, Error};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

/// Standard Response to signal if a request was successfull or not
#[derive(Deserialize, Serialize, ToSchema)]
pub struct Response {
    #[schema(example = true)]
    success: bool,
}

#[derive(Debug, Display, Error)]
pub enum ServerError {
    #[display(fmt = "Internal Error")]
    InternalError,

    #[display(fmt = "Bad Request")]
    BadClientData,

    #[display(fmt = "Unauthorized")]
    Unauthorized,

    #[display(fmt = "Forbidden")]
    Forbidden,

    #[display(fmt = "Conflict")]
    Conflict,
}

impl error::ResponseError for ServerError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            ServerError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::BadClientData => StatusCode::BAD_REQUEST,
            ServerError::Unauthorized => StatusCode::UNAUTHORIZED,
            ServerError::Forbidden => StatusCode::FORBIDDEN,
            ServerError::Conflict => StatusCode::CONFLICT,
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        run::travel_submit_run_v1,
        run::travel_submit_run_v2,
        run::travel_file_upload,
        run::submit_gps_live,
        run::terminate_run,
        user::user_login,
        user::user_create
    ),
    components(schemas(
        Response,
        user::UserCreation,
        user::UserLogin,
        run::SubmitTravelV1,
        run::SubmitTravelV2,
        run::SubmitGpsPoint
    ))
)]
pub struct ApiDoc;
