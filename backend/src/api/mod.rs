use std::net::SocketAddr;

use axum::http::{header, Method};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::nhl_api::nhl::NhlClient;
use crate::FantasyDb;

pub mod dtos;
pub mod handlers;
pub mod response;
pub mod routes;

pub const SEASON: u32 = 20252026;
pub const GAME_TYPE: u8 = 3;

pub async fn run_server(
    db: FantasyDb,
    nhl_client: NhlClient,
    jwt_secret: String,
    port: u16,
) -> anyhow::Result<()> {
    // Create CORS middleware
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_origin(Any);

    // Build our application with routes
    let app = routes::create_router(db, nhl_client, jwt_secret).layer(cors);

    // Create a TCP listener for our address
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Starting server on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
