use axum::http::Request;
use axum::response::{IntoResponse, Response};
use google_sheets4::api::Color;
use reqwest::StatusCode;
use tower_http::trace::OnRequest;
use tracing::Span;

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
    fn into_app_err(self) -> AppError;
}

impl IntoAppError for (StatusCode, String) {
    fn into_app_err(self) -> AppError {
        AppError::StatusCode(self.0, self.1)
    }
}

#[derive(Copy, Clone)]
pub struct RequestTracer;

impl<B> OnRequest<B> for RequestTracer {
    fn on_request(&mut self, request: &Request<B>, _: &Span) {
        tracing::info!("{} {}", request.method(), request.uri());
    }
}

pub trait ToHex {
    fn to_hex(&self) -> u32;
}

impl ToHex for Color {
    fn to_hex(&self) -> u32 {
        let red = (self.red.unwrap() * 255.0) as u8;
        let green = (self.green.unwrap() * 255.0) as u8;
        let blue = (self.blue.unwrap() * 255.0) as u8;

        let hex = format!("{:02x}{:02x}{:02x}", red, green, blue);

        u32::from_str_radix(&hex, 16).unwrap()
    }
}
