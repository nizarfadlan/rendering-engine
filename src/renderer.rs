use base64::{Engine as _, engine::general_purpose};
use headless_chrome::{Browser, LaunchOptions, protocol::cdp::Page};
use anyhow::{Result, anyhow};
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use crate::models::RenderRequest;
use crate::template;

#[derive(Clone)]
pub struct RenderingEngine {
    browser: Arc<Mutex<Option<Browser>>>,
    launch_options: LaunchOptions<'static>,
}

impl RenderingEngine {
    pub fn new() -> Result<Self> {
        let launch_options = LaunchOptions::default_builder()
            .headless(true)
            .sandbox(false)
            .args(vec![
                OsStr::new("--no-sandbox"),
                OsStr::new("--disable-setuid-sandbox"),
                OsStr::new("--disable-dev-shm-usage"),
                OsStr::new("--disable-gpu"),
            ])
            .build()
            .map_err(|_| anyhow!("Could not find Chrome/Chromium binary"))?;

        let browser = Browser::new(launch_options.clone())?;

        Ok(Self {
            browser: Arc::new(Mutex::new(Some(browser))),
            launch_options,
        })
    }

    fn get_or_create_browser(&self) -> Result<Browser> {
        let mut browser_lock = self.browser.lock().unwrap();

        // Check if browser exists and is alive
        if let Some(ref browser) = *browser_lock {
            // Try to get tabs to check if browser is still alive
            if let Ok(_) = browser.new_tab() {
                return Ok(browser.clone());
            }

            // Browser is dead, log and clear
            tracing::warn!("Browser health check failed, will recreate");
            *browser_lock = None;
        }

        // Browser is dead or doesn't exist, create new one
        tracing::warn!("Browser crashed or not available, creating new instance");
        let new_browser = Browser::new(self.launch_options.clone())?;
        *browser_lock = Some(new_browser.clone());

        Ok(new_browser)
    }

    pub async fn render(&self, request: &RenderRequest) -> Result<Vec<u8>> {
        let engine = self.clone();
        let request = request.clone();

        tokio::task::spawn_blocking(move || {
            engine.render_sync(&request)
        })
        .await
        .map_err(|e| anyhow!("Task join error: {}", e))?
    }


    fn render_sync(&self, request: &RenderRequest) -> Result<Vec<u8>> {
        let html = template::generate_html(request)?;

        let browser = self.get_or_create_browser()
            .or_else(|e| {
                tracing::warn!("First browser creation failed: {}, retrying...", e);
                // Force clear and retry once
                *self.browser.lock().unwrap() = None;
                self.get_or_create_browser()
            })?;
        let tab = browser.new_tab()?;

        // Set viewport
        tab.set_bounds(headless_chrome::types::Bounds::Normal {
            left: Some(0),
            top: Some(0),
            width: Some(request.options.width as f64),
            height: Some(request.options.height as f64),
        })?;

        // Navigate to HTML
        let data_url = format!(
            "data:text/html;base64,{}",
            general_purpose::STANDARD.encode(&html)
        );
        tab.navigate_to(&data_url)?;

        // Get library template
        let library_template = crate::registry::LIBRARY_REGISTRY
            .get(&request.library.name)
            .ok_or_else(|| anyhow!("Unsupported library: {}", request.library.name))?;

        // Wait for container element
        tab.wait_for_element_with_custom_timeout(
            &library_template.wait_selector,
            Duration::from_secs(10)
        )?;

        // Wait for render ready signal
        let mut attempts = 0;
        let max_attempts = 50;

        while attempts < max_attempts {
            let ready: bool = tab
                .evaluate("window.renderReady === true", false)?
                .value
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if ready {
                break;
            }

            let error: Option<String> = tab
                .evaluate("window.renderError", false)?
                .value
                .and_then(|v| v.as_str().map(String::from));

            if let Some(err) = error {
                return Err(anyhow!("Render initialization failed: {}", err));
            }

            sleep(Duration::from_millis(100));
            attempts += 1;
        }

        if attempts >= max_attempts {
            return Err(anyhow!("Timeout waiting for render to complete"));
        }

        sleep(Duration::from_millis(500));

        // Capture based on format
        let result = match request.options.format.as_str() {
            "png" => {
                let quality = request.options.quality.unwrap_or(90) as i64;
                tab.capture_screenshot(
                    Page::CaptureScreenshotFormatOption::Png,
                    Some(quality.try_into().unwrap()),
                    None,
                    true,
                )?
            }
            "jpeg" | "jpg" => {
                let quality = request.options.quality.unwrap_or(90) as i64;
                tab.capture_screenshot(
                    Page::CaptureScreenshotFormatOption::Jpeg,
                    Some(quality.try_into().unwrap()),
                    None,
                    true,
                )?
            }
            "pdf" => {
                tab.print_to_pdf(None)?
            }
            _ => {
                return Err(anyhow!("Unsupported format: {}", request.options.format));
            }
        };

        Ok(result)
    }
}
