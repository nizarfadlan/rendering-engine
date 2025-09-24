use poem_openapi::Object;
use serde::{Serialize, Deserialize};

#[derive(Object, Serialize, Deserialize, Debug, Clone)]
pub struct RenderRequest {
    /// Library configuration
    pub library: LibraryConfig,

    /// Chart data in library-specific format
    pub data: serde_json::Value,

    /// Rendering options
    pub options: RenderOptions,
}

#[derive(Object, Serialize, Deserialize, Debug, Clone)]
pub struct LibraryConfig {
    /// Library name (e.g., "apache-echarts", "chartjs")
    pub name: String,

    /// Library version
    pub version: String,

    /// Custom CDN URL (optional)
    pub cdn_url: Option<String>,
}

#[derive(Object, Serialize, Deserialize, Debug, Clone)]
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
}
