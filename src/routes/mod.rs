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
    #[display(fmt = "internal error")]
    InternalError,

    #[display(fmt = "bad request")]
    BadClientData,
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
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        run::travel_submit_run,
        run::travel_file_upload,
        run::travel_list,
        user::user_login,
        user::user_create
    ),
    components(schemas(Response, user::UserCreation, user::UserLogin, run::SubmitTravel))
)]
pub struct ApiDoc;
