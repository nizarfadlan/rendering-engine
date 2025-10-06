use poem_openapi::{
    ApiResponse, Object,
    payload::{Attachment, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{InternalServerErrorResponse, UnauthorizedResponse};

#[derive(Object, Deserialize, Clone)]
pub struct LibraryConfig {
    /// Library name (e.g., "apache-echarts", "chartjs")
    pub name: String,

    /// Library version
    pub version: String,

    /// Custom CDN URL (optional)
    pub cdn_url: Option<String>,
}

#[derive(Object, Deserialize, Clone)]
pub struct RenderOptions {
    /// Image width in pixels
    #[oai(validator(minimum(value = "100"), maximum(value = "4000")))]
    pub width: u32,

    /// Image height in pixels
    #[oai(validator(minimum(value = "100"), maximum(value = "4000")))]
    pub height: u32,

    /// Output format (png, jpeg, pdf)
    #[oai(validator(pattern = "^(png|jpeg|jpg|pdf)$"))]
    pub format: String,

    /// Image quality for JPEG (1-100)
    #[oai(validator(minimum(value = "1"), maximum(value = "100")))]
    pub quality: Option<u8>,

    /// Device scale factor for high-DPI displays
    #[oai(validator(minimum(value = "0.5"), maximum(value = "3.0")))]
    pub device_scale_factor: Option<f64>,

    /// Custom delay after render ready (milliseconds)
    /// Default: 500ms
    #[oai(validator(minimum(value = "0"), maximum(value = "5000")))]
    pub render_delay_ms: Option<u64>,

    /// Polling interval for checking render status (milliseconds)
    /// Default: 100ms
    #[oai(validator(minimum(value = "50"), maximum(value = "1000")))]
    pub poll_interval_ms: Option<u64>,

    /// Maximum render timeout (milliseconds)
    /// Default: 30000ms (30 seconds)
    #[oai(validator(minimum(value = "1000"), maximum(value = "60000")))]
    pub timeout_ms: Option<u64>,

    /// Return base64 encoded string instead of binary
    pub return_base64: Option<bool>,
}

#[derive(Object, Deserialize, Clone)]
pub struct RenderRequest {
    pub library: LibraryConfig,
    pub data: JsonValue,
    pub options: RenderOptions,
}

#[derive(Object, Serialize)]
pub struct Base64Response {
    /// Base64 encoded image data
    pub data: String,

    /// MIME type of the image
    pub mime_type: String,
}

#[derive(ApiResponse)]
pub enum RenderResponse {
    #[oai(status = 200, content_type = "application/octet-stream")]
    Binary(Attachment<Vec<u8>>),

    #[oai(status = 200, content_type = "application/json")]
    Base64(Json<Base64Response>),

    #[oai(status = 401)]
    Unauthorized(Json<UnauthorizedResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<InternalServerErrorResponse>),
}

#[derive(ApiResponse)]
pub enum ListLibrariesResponse {
    #[oai(status = 200, content_type = "application/json")]
    Ok(Json<Vec<LibraryConfig>>),

    #[oai(status = 401)]
    Unauthorized(Json<UnauthorizedResponse>),

    #[oai(status = 500)]
    InternalServerError(Json<InternalServerErrorResponse>),
}
