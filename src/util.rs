use axum::response::{IntoResponse, Response};
use reqwest::StatusCode;

const UNITS: &[&str] = &["", "k", "M", "B", "T"];

pub fn format_number(n: usize) -> String {
    let mut n = n as f32;

    for unit in UNITS {
        if n < 1000.0 {
            return format!("{n:.1}{unit}");
        } else {
            n /= 1000.0;
        }
    }

    let last = UNITS.last().unwrap();
    format!("{n:.1}{last}")
}

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
                    format!("Something went wrong: {}", e),
                )
                    .into_response(),
                Some(e) => {
                    let code = if let Some(status) = e.status() {
                        status
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    };

                    (code, format!("{}", e)).into_response()
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

impl<'a> IntoAppError for (StatusCode, &'a str) {
    fn into_app_err<T>(self) -> Result<T, AppError> {
        Err(AppError::StatusCode(self.0, self.1.into()))
    }
}
