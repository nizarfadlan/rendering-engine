use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use headless_chrome::Tab;
use headless_chrome::{Browser, LaunchOptions, protocol::cdp::Page};
use std::ffi::OsStr;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use crate::core::registry::LIBRARY_REGISTRY;
use crate::core::template;
use crate::schemas::render::{RenderRequest, Base64Response};

struct TabGuard {
    tab: Arc<Tab>,
}

impl TabGuard {
    fn new(tab: Arc<Tab>) -> Self {
        Self { tab }
    }

    fn as_ref(&self) -> &Arc<Tab> {
        &self.tab
    }
}

impl Drop for TabGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tab.close(true) {
            tracing::warn!("Failed to close tab during cleanup: {}", e);
        } else {
            tracing::debug!("Tab closed successfully");
        }
    }
}

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
                OsStr::new("--disable-software-rasterizer"),
                OsStr::new("--disable-extensions"),
                OsStr::new("--disable-background-networking"),
                OsStr::new("--disable-sync"),
                OsStr::new("--metrics-recording-only"),
                OsStr::new("--mute-audio"),
                OsStr::new("--no-first-run"),
                OsStr::new("--disable-default-apps"),
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
            match browser.new_tab() {
                Ok(tab) => {
                    // Close test tab immediately
                    let _ = tab.close(true);
                    return Ok(browser.clone());
                }
                Err(_) => {
                    tracing::warn!("Browser health check failed, recreating");
                    *browser_lock = None;
                }
            }
        }

        // Browser is dead or doesn't exist, create new one
        tracing::warn!("Browser crashed or not available, creating new instance");
        let new_browser = Browser::new(self.launch_options.clone())?;
        *browser_lock = Some(new_browser.clone());

        Ok(new_browser)
    }

    pub async fn render(&self, request: RenderRequest) -> Result<Vec<u8>> {
        let engine = self.clone();

        tokio::task::spawn_blocking(move || engine.render_sync(&request))
            .await
            .map_err(|e| anyhow!("Task join error: {}", e))?
    }

    pub async fn render_base64(&self, request: RenderRequest) -> Result<Base64Response> {
        let result = self.render(request.clone()).await?;

        let mime_type = match request.options.format.as_str() {
            "png" => "image/png",
            "jpeg" | "jpg" => "image/jpeg",
            "pdf" => "application/pdf",
            _ => "application/octet-stream",
        };

        Ok(Base64Response {
            data: general_purpose::STANDARD.encode(&result),
            mime_type: mime_type.to_string(),
        })
    }

    fn render_sync(&self, request: &RenderRequest) -> Result<Vec<u8>> {
        let html = template::generate_html(request)?;

        let browser = self.get_or_create_browser().or_else(|e| {
            tracing::warn!("First browser creation failed: {}, retrying...", e);
            // Force clear and retry once
            *self.browser.lock().unwrap() = None;
            self.get_or_create_browser()
        })?;
        let tab = browser.new_tab()?;
        let tab_guard = TabGuard::new(tab);
        let tab = tab_guard.as_ref();

        // Set viewport
        let scale_factor = request.options.device_scale_factor.unwrap_or(1.0);
        tab.set_bounds(headless_chrome::types::Bounds::Normal {
            left: Some(0),
            top: Some(0),
            width: Some(request.options.width as f64),
            height: Some(request.options.height as f64),
        })?;

        if scale_factor != 1.0 {
            tab.call_method(
                headless_chrome::protocol::cdp::Emulation::SetDeviceMetricsOverride {
                    width: request.options.width,
                    height: request.options.height,
                    device_scale_factor: scale_factor,
                    mobile: false,
                    scale: Some(scale_factor),
                    screen_width: Some(request.options.width),
                    screen_height: Some(request.options.height),
                    position_x: Some(0),
                    position_y: Some(0),
                    dont_set_visible_size: None,
                    screen_orientation: None,
                    viewport: None,
                    display_feature: None,
                    device_posture: None,
                },
            )?;
        }

        // Navigate to HTML
        let data_url = format!(
            "data:text/html;base64,{}",
            general_purpose::STANDARD.encode(&html)
        );
        tab.navigate_to(&data_url)?;

        // Get library template
        let library_template = LIBRARY_REGISTRY
            .get(&request.library.name)
            .ok_or_else(|| anyhow!("Unsupported library: {}", request.library.name))?;

        // Wait for container element
        tab.wait_for_element_with_custom_timeout(
            &library_template.wait_selector,
            Duration::from_secs(10),
        )?;

        // Wait for render ready signal
        self.wait_for_render_ready(tab, request)?;

        // Capture based on format
        let result = self.capture_screenshot(tab, request)?;

        Ok(result)
    }

    fn wait_for_render_ready(&self, tab: &Arc<Tab>, request: &RenderRequest) -> Result<()> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 50;
        const POLL_INTERVAL_MS: u64 = 100;
        let poll_interval = Duration::from_millis(POLL_INTERVAL_MS);

        while attempts < MAX_ATTEMPTS {
            let ready: bool = tab
                .evaluate("window.renderReady === true", false)?
                .value
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if ready {
                tracing::debug!("Render ready after {} attempts", attempts);
                break;
            }

            let error: Option<String> = tab
                .evaluate("window.renderError", false)?
                .value
                .and_then(|v| v.as_str().map(String::from));

            if let Some(err) = error {
                return Err(anyhow!("Render initialization failed: {}", err));
            }

            sleep(poll_interval);
            attempts += 1;
        }

        if attempts >= MAX_ATTEMPTS {
            return Err(anyhow!(
                "Timeout waiting for render to complete after {} attempts",
                MAX_ATTEMPTS
            ));
        }

        let render_delay = Duration::from_millis(POLL_INTERVAL_MS);
        sleep(render_delay);

        Ok(())
    }

    fn capture_screenshot(&self, tab: &Arc<Tab>, request: &RenderRequest) -> Result<Vec<u8>> {
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
