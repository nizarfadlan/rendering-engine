use std::sync::Arc;

use poem::{
    EndpointExt, Route,
    middleware::{AddData, AddDataEndpoint, Cors, CorsEndpoint},
};
use poem_openapi::OpenApiService;

use core::renderer::RenderingEngine;
use settings::Config;

use crate::routes::render::ApiRender;

pub mod core;
pub mod routes;
pub mod schemas;
pub mod settings;

pub struct AppState {
    pub engine: Arc<RenderingEngine>,
}

pub fn init_openapi_route(
    app_state: Arc<AppState>,
    config: &Config,
) -> CorsEndpoint<AddDataEndpoint<Route, Arc<AppState>>> {
    let prefix = config.prefix.clone().unwrap_or("/".to_string());
    let openapi_route =
        OpenApiService::new(ApiRender, "Renderer Engine API", "1.0").server(prefix.clone());

    let openapi_json_endpoint = openapi_route.spec_endpoint();
    let ui = openapi_route.swagger_ui();
    Route::new()
        .nest(prefix, openapi_route)
        .nest("/docs", ui)
        .at("openapi.json", openapi_json_endpoint)
        .with(AddData::new(app_state))
        .with(Cors::new())
}
