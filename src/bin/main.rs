use poem::listener::TcpListener;
use rendering_engine::core::renderer::RenderingEngine;
use rendering_engine::settings::get_config;
use rendering_engine::{AppState, init_openapi_route};
use tracing::Level;

use std::sync::Arc;
use tracing_subscriber;

#[tokio::main]
async fn main() {
    let log_level = Level::DEBUG;
    // Logging to File
    let file_appender = tracing_appender::rolling::daily("./logs", "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_max_level(log_level)
        .init();

    tracing::info!("Initializing Rendering Service...");

    let config = get_config();
    tracing::info!("run with config: {:?}", config);

    let engine = Arc::new(RenderingEngine::new().expect("Failed to initialize rendering engine"));

    // Init App State
    let app_state = Arc::new(AppState { engine: engine });

    tracing::info!("Rendering engine initialized successfully");

    let app = init_openapi_route(app_state.clone(), &config);
    tracing::info!("run server on {}:{}", config.host, config.port);
    poem::Server::new(TcpListener::bind(format!(
        "{}:{}",
        config.host, config.port
    )))
    .run(app)
    .await
    .unwrap()
}
