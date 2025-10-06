use std::sync::Arc;

use poem::web::Data;
use poem_openapi::{
    OpenApi, Tags,
    payload::{Attachment, Json},
};

use crate::{
    AppState,
    core::registry::LIBRARY_REGISTRY,
    schemas::{
        common::InternalServerErrorResponse,
        render::{LibraryConfig, ListLibrariesResponse, RenderRequest, RenderResponse},
    },
};

#[derive(Tags)]
enum ApiRenderTags {
    Render,
}

pub struct ApiRender;

#[OpenApi()]
impl ApiRender {
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
    #[oai(path = "/render", method = "post", tag = "ApiRenderTags::Render")]
    async fn render(
        &self,
        Json(json): Json<RenderRequest>,
        state: Data<&Arc<AppState>>,
    ) -> RenderResponse {
        tracing::info!(
            "Rendering: library={}, size={}x{}",
            json.library.name,
            json.options.width,
            json.options.height
        );

        let return_base64 = json.options.return_base64.unwrap_or(false);

        if return_base64 {
            let result = match state.engine.render_base64(json).await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Render error: {}", e);
                    return RenderResponse::InternalServerError(Json(
                        InternalServerErrorResponse::new(
                            "route.render",
                            "render",
                            "Rendering failed",
                            &e.to_string(),
                        ),
                    ));
                }
            };

            RenderResponse::Base64(Json(result))
        } else {
            let result = match state.engine.render(json).await {
                Ok(res) => res,
                Err(e) => {
                    tracing::error!("Render error: {}", e);
                    return RenderResponse::InternalServerError(Json(
                        InternalServerErrorResponse::new(
                            "route.render",
                            "render",
                            "Rendering failed",
                            &e.to_string(),
                        ),
                    ));
                }
            };

            RenderResponse::Binary(Attachment::new(result))
        }
    }

    /// List Supported Libraries
    ///
    /// Get list of all supported libraries
    #[oai(path = "/libraries", method = "get")]
    async fn list_libraries(&self) -> ListLibrariesResponse {
        let libraries = LIBRARY_REGISTRY
            .iter()
            .map(|(name, template)| LibraryConfig {
                name: name.clone(),
                version: "latest".to_string(),
                cdn_url: Some(template.cdn_url.clone()),
            })
            .collect();

        ListLibrariesResponse::Ok(Json(libraries))
    }

    #[oai(path = "/health", method = "get")]
    async fn health(&self, state: Data<&Arc<AppState>>) -> Json<serde_json::Value> {
        let status = state.engine.health_check();

        Json(serde_json::json!({
            "status": "healthy",
            "browser_pool": {
                "available": status.pool_size,
                "capacity": status.total_capacity,
                "utilization_pct": ((status.total_capacity - status.pool_size) as f64 / status.total_capacity as f64 * 100.0)
            },
            "render_slots": {
                "available": status.available_permits,
                "capacity": status.max_concurrent,
                "utilization_pct": ((status.max_concurrent - status.available_permits) as f64 / status.max_concurrent as f64 * 100.0)
            }
        }))
    }
}
