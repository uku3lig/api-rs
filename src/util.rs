use axum::http::Request;
use axum::response::{IntoResponse, Response};
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

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self.0.downcast_ref::<reqwest::Error>() {
            None => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Something went wrong: {}", self.0),
            )
                .into_response(),
            Some(e) => {
                let code = if let Some(status) = e.status() {
                    status
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };

                (code, format!("{}", self.0)).into_response()
            }
        }
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

#[derive(Copy, Clone)]
pub struct RequestTracer;

impl<B> OnRequest<B> for RequestTracer {
    fn on_request(&mut self, request: &Request<B>, _: &Span) {
        tracing::info!("{} {}", request.method(), request.uri());
    }
}
