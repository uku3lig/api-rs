use std::time::Instant;

use axum::{Router, extract::Request, middleware::Next, response::IntoResponse, routing::get};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use reqwest::header::USER_AGENT;
use tokio::signal::unix::{SignalKind, signal};

const TOTAL_REQS_KEY: &str = "api_rs_requests_total";
const REQ_DURATION_KEY: &str = "api_rs_request_duration_seconds";

const EXPONENTIAL_SECONDS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

pub async fn start_metrics_app(socket_addr: String) -> anyhow::Result<()> {
    let handle = PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Suffix("duration_seconds".to_string()),
            EXPONENTIAL_SECONDS,
        )?
        .install_recorder()?;

    let app = Router::new().route("/metrics", get(move || std::future::ready(handle.render())));

    let listener = tokio::net::TcpListener::bind(socket_addr).await?;
    tracing::info!("metrics listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            signal(SignalKind::terminate()).unwrap().recv().await;
        })
        .await
        .map_err(anyhow::Error::from)
}

pub async fn track(request: Request, next: Next) -> impl IntoResponse {
    let method = request.method().clone();
    let path = {
        let mut p = request.uri().path();

        if p.contains("profile") {
            p = p.rsplit_once('/').unwrap().0;
        }

        p.to_owned()
    };
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|a| a.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();

    let start = Instant::now();
    let response = next.run(request).await;
    let delta_time = start.elapsed().as_secs_f64();

    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
        ("user_agent", user_agent),
    ];

    metrics::counter!(TOTAL_REQS_KEY, &labels).increment(1);
    metrics::histogram!(REQ_DURATION_KEY, &labels).record(delta_time);

    response
}
