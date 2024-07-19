use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

const UNITS: &[&str] = &["", "k", "M", "B", "T"];

pub fn format_number(n: u32) -> String {
    let mut n = f64::from(n);

    for unit in UNITS {
        if n < 1000.0 {
            return format!("{n:.1}{unit}");
        }

        n /= 1000.0;
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

impl<'a> IntoAppError for (StatusCode, &'a str) {
    fn into_app_err<T>(self) -> Result<T, AppError> {
        Err(AppError::StatusCode(self.0, self.1.into()))
    }
}
