use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let app = Router::new();

    let socket_addr = SocketAddr::from_str("0.0.0.0:3000")?;
    tracing::info!("listening on {}", socket_addr);

    Ok(axum::Server::bind(&socket_addr)
        .serve(app.into_make_service())
        .await?)
}
