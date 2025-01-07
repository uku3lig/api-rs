use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub enum AppError {
    Anyhow(anyhow::Error),
    StatusCode(StatusCode, String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            Self::StatusCode(c, m) => (c, m).into_response(),
            Self::Anyhow(e) => match e.downcast_ref::<reqwest::Error>() {
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Something went wrong: {e}"),
                )
                    .into_response(),
                Some(e) => {
                    let code = e.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                    (code, format!("{e}")).into_response()
                }
            },
        }
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self::Anyhow(value.into())
    }
}

pub trait IntoAppError {
    fn into_app_err<T>(self) -> Result<T, AppError>;
}

impl IntoAppError for (StatusCode, &str) {
    fn into_app_err<T>(self) -> Result<T, AppError> {
        Err(AppError::StatusCode(self.0, self.1.into()))
    }
}
