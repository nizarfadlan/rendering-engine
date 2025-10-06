use poem::{http::StatusCode, test::TestClient};
use rendering_engine::{init_openapi_route, settings::get_config, AppState};
use rendering_engine::core::renderer::RenderingEngine;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_browser_pool_auto_scaling() {
    // Initialize rendering engine with small pool
    let engine = Arc::new(
        RenderingEngine::with_config(1, 10, 20)
            .expect("Failed to initialize rendering engine")
    );

    let app_state = Arc::new(AppState {
        engine: engine.clone()
    });

    let config = get_config();
    let app = init_openapi_route(app_state.clone(), &config);
    let cli = TestClient::new(app);

    // Step 1: Check initial pool status
    let resp = cli.get("/health").send().await;
    resp.assert_status_is_ok();

    let body = resp.0.into_body().into_string().await.unwrap();
    let health: Value = serde_json::from_str(&body).unwrap();
    let initial_pool_size = health["browser_pool"]["available"].as_u64().unwrap();

    println!("Initial pool size: {}", initial_pool_size);
    assert_eq!(initial_pool_size, 1, "Pool should start with 1 instance");

    // Step 2: Send multiple concurrent requests to trigger scaling
    let mut handles = vec![];
    for i in 1..=8 {
        let app_state_clone = app_state.clone();
        let config_clone = config.clone();
        let handle = tokio::spawn(async move {
            let app = init_openapi_route(app_state_clone, &config_clone);
            let client = TestClient::new(app);
            let payload = json!({
                "library": {
                    "name": "apache-echarts",
                    "version": "5.4.0"
                },
                "data": {
                    "title": {"text": format!("Test Chart {}", i)},
                    "xAxis": {"data": ["A", "B", "C"]},
                    "yAxis": {},
                    "series": [{"type": "bar", "data": [10, 20, 30]}]
                },
                "options": {
                    "width": 800,
                    "height": 600,
                    "format": "png"
                }
            });

            let resp = client
                .post("/render")
                .content_type("application/json")
                .body_json(&payload)
                .send()
                .await;

            if resp.0.status() != StatusCode::OK {
                let err_body = resp.0.into_body().into_string().await.unwrap();
                panic!("Request {} failed: {}", i, err_body);
            }

            println!("Request {} status: {}", i, resp.0.status());
            resp
        });
        handles.push(handle);
    }

    sleep(Duration::from_millis(1000)).await;

    // Step 3: Check pool status during load
    let resp = cli.get("/health").send().await;
    resp.assert_status_is_ok();

    let body = resp.0.into_body().into_string().await.unwrap();
    let health: Value = serde_json::from_str(&body).unwrap();
    let pool_size_during = health["browser_pool"]["available"].as_u64().unwrap();

    println!("Pool size during load: {} (initial was: {})", pool_size_during, initial_pool_size);

    for handle in handles {
        let _ = handle.await;
    }

    sleep(Duration::from_millis(1000)).await;

    // Step 4: Check final pool status
    let resp = cli.get("/health").send().await;
    resp.assert_status_is_ok();

    let body = resp.0.into_body().into_string().await.unwrap();
    let health: Value = serde_json::from_str(&body).unwrap();
    let final_pool_size = health["browser_pool"]["available"].as_u64().unwrap();

    // Assertions
    println!("Initial pool size: {}", initial_pool_size);
    println!("Final pool size: {}", final_pool_size);

    assert!(
        final_pool_size > initial_pool_size,
        "Pool should have scaled up from {} to {}",
        initial_pool_size,
        final_pool_size
    );

    assert!(
        final_pool_size <= 10,
        "Pool should not exceed max size of 10"
    );

    println!("\nAuto-scaling test PASSED!");
    println!("   - Started with {} instance(s)", initial_pool_size);
    println!("   - Scaled up to {} instance(s)", final_pool_size);
}

#[tokio::test]
async fn test_health_endpoint_shows_pool_metrics() {
    let engine = Arc::new(
        RenderingEngine::new()
            .expect("Failed to initialize rendering engine")
    );

    let app_state = Arc::new(AppState { engine });
    let config = get_config();
    let app = init_openapi_route(app_state, &config);
    let cli = TestClient::new(app);

    let resp = cli.get("/health").send().await;
    resp.assert_status(StatusCode::OK);

    let body = resp.0.into_body().into_string().await.unwrap();
    let health: Value = serde_json::from_str(&body).unwrap();

    // Check that health response has expected fields
    assert!(health["browser_pool"]["available"].is_number());
    assert!(health["browser_pool"]["capacity"].is_number());
    assert!(health["render_slots"]["available"].is_number());
    assert_eq!(health["status"].as_str().unwrap(), "healthy");

    println!("Health endpoint test PASSED!");
}
