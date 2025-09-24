mod models;
mod template;
mod renderer;
mod registry;

use poem::{
    listener::TcpListener,
    web::Data,
    EndpointExt,
    Route,
    Server,
    Result as PoemResult,
    Error as PoemError,
    http::StatusCode,
};
use poem_openapi::payload::{Attachment};
use poem_openapi::{ApiResponse, OpenApi, OpenApiService};

use std::sync::Arc;
use tracing_subscriber;

use crate::models::RenderRequest;
use crate::renderer::RenderingEngine;

#[derive(ApiResponse)]
enum RenderResponse {
    /// PNG Image
    #[oai(status = 200, content_type = "image/png")]
    Png(Attachment<Vec<u8>>),

    /// JPEG Image
    #[oai(status = 200, content_type = "image/jpeg")]
    Jpeg(Attachment<Vec<u8>>),

    /// PDF Document
    #[oai(status = 200, content_type = "application/pdf")]
    Pdf(Attachment<Vec<u8>>),

    /// Unknown format
    #[oai(status = 200, content_type = "application/octet-stream")]
    Unknown(Attachment<Vec<u8>>),
}

struct RenderApi;

#[OpenApi()]
impl RenderApi {
    /// Render
    ///
    /// Generate a image from configuration using headless browser.
    /// Supports multiple libraries including ECharts, Chart.js, and Konva.js.
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "library": {
    ///     "name": "apache-echarts",
    ///     "version": "5.4.0"
    ///   },
    ///   "data": {
    ///     "title": { "text": "Sales" },
    ///     "xAxis": { "data": ["Mon", "Tue", "Wed"] },
    ///     "yAxis": {},
    ///     "series": [{ "type": "bar", "data": [120, 200, 150] }]
    ///   },
    ///   "options": {
    ///     "width": 800,
    ///     "height": 600,
    ///     "format": "png"
    ///   }
    /// }
    /// ```
    #[oai(path = "/render", method = "post")]
    async fn render(&self, engine: Data<&Arc<RenderingEngine>>, req: poem_openapi::payload::Json<RenderRequest>) -> PoemResult<RenderResponse> {
        tracing::info!(
            "Rendering: library={}, size={}x{}",
            req.library.name,
            req.options.width,
            req.options.height
        );

        let result = engine
            .render(&req.0)
            .await
            .map_err(|e| {
                tracing::error!("Render error: {}", e);
                PoemError::from_string(
                    format!("Rendering failed: {}", e),
                    StatusCode::INTERNAL_SERVER_ERROR
                )
            })?;

        tracing::info!("Render completed successfully, size: {} bytes", result.len());

        let response = match req.options.format.as_str() {
            "png" => RenderResponse::Png(Attachment::new(result)),
            "jpeg" | "jpg" => RenderResponse::Jpeg(Attachment::new(result)),
            "pdf" => RenderResponse::Pdf(Attachment::new(result)),
            _ => RenderResponse::Unknown(Attachment::new(result)),
        };

        Ok(response)
    }

    /// List Supported Libraries
    ///
    /// Get list of all supported libraries
    #[oai(path = "/libraries", method = "get")]
    async fn list_libraries(
        &self,
    ) -> poem_openapi::payload::Json<Vec<String>> {
        let libraries = crate::registry::LIBRARY_REGISTRY
            .keys()
            .map(|k| k.to_string())
            .collect();

        poem_openapi::payload::Json(libraries)
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    tracing::info!("Initializing Rendering Service...");

    let engine = Arc::new(
        RenderingEngine::new()
            .expect("Failed to initialize rendering engine")
    );

    tracing::info!("Rendering engine initialized successfully");

    let api_service = OpenApiService::new(
        RenderApi,
        "Rendering Service",
        "1.0.0",
    )
    .server("http://localhost:8000")
    .description(
        "Render as images using headless browser. \
         Supports ECharts, Chart.js, Konva.js, and D3.js"
    );

    let ui = api_service.swagger_ui();
    let spec = api_service.spec_endpoint();

    let app = Route::new()
        .nest("/", api_service)
        .nest("/docs", ui)
        .nest("/spec", spec)
        .data(engine);

    println!("\n🚀 Rendering Service Started!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📖 API Docs:     http://localhost:8000/docs");
    println!("📄 OpenAPI Spec: http://localhost:8000/spec");
    println!("📚 Libraries:    http://localhost:8000/api/libraries");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Server::new(TcpListener::bind("0.0.0.0:8000"))
        .run(app)
        .await
}
