use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use crossbeam::queue::ArrayQueue;
use headless_chrome::Tab;
use headless_chrome::{Browser, LaunchOptions, protocol::cdp::Page};
use parking_lot::RwLock;
use std::ffi::OsStr;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

use crate::core::registry::LIBRARY_REGISTRY;
use crate::core::template;
use crate::schemas::render::{Base64Response, RenderRequest};

const MIN_POOL_SIZE: usize = 1;
const MAX_POOL_SIZE: usize = 10;
const MAX_CONCURRENT_RENDERS: usize = 20;
const SCALE_UP_THRESHOLD: f32 = 0.8; // Scale up when 80% capacity used

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub pool_size: usize,
    pub total_capacity: usize,
    pub available_permits: usize,
    pub max_concurrent: usize,
}

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

struct BrowserInstance {
    browser: Browser,
    last_health_check: Arc<RwLock<Instant>>,
}

impl BrowserInstance {
    fn new(launch_options: &LaunchOptions<'static>) -> Result<Self> {
        let browser = Browser::new(launch_options.clone())?;
        let now = Instant::now();

        Ok(Self {
            browser,
            last_health_check: Arc::new(RwLock::new(now)),
        })
    }

    fn is_healthy(&self) -> bool {
        match self.browser.get_version() {
            Ok(_) => {
                *self.last_health_check.write() = Instant::now();
                true
            }
            Err(_) => false,
        }
    }

    fn new_tab(&self) -> Result<Arc<Tab>> {
        self.browser
            .new_tab()
            .map_err(|e| anyhow!("Failed to create tab: {}", e))
    }
}

struct BrowserPoolGuard {
    pool: Arc<BrowserPool>,
    instance: Option<Arc<BrowserInstance>>,
}

impl Drop for BrowserPoolGuard {
    fn drop(&mut self) {
        if let Some(instance) = self.instance.take() {
            self.pool.release(instance);
        }
    }
}

struct BrowserPool {
    pool: ArrayQueue<Arc<BrowserInstance>>,
    launch_options: LaunchOptions<'static>,
    max_size: usize,
    current_size: Arc<RwLock<usize>>,
}

impl BrowserPool {
    fn new(min_size: usize, max_size: usize, launch_options: LaunchOptions<'static>) -> Result<Self> {
        let pool = ArrayQueue::new(max_size);

        // Start with minimum pool size
        for i in 0..min_size {
            match BrowserInstance::new(&launch_options) {
                Ok(instance) => {
                    if pool.push(Arc::new(instance)).is_err() {
                        tracing::error!("Failed to push browser {} to pool", i);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create browser instance {}: {}", i, e);
                }
            }
        }

        if pool.len() == 0 {
            return Err(anyhow!("Failed to initialize browser pool"));
        }

        let initial_count = pool.len();

        tracing::info!(
            "Browser pool initialized with {}/{} instances (max: {})",
            initial_count,
            min_size,
            max_size
        );

        Ok(Self {
            pool,
            launch_options,
            max_size,
            current_size: Arc::new(RwLock::new(initial_count)),
        })
    }

    fn acquire(&self) -> Result<Arc<BrowserInstance>> {
        // Try to get from pool first
        if let Some(instance) = self.pool.pop() {
            if instance.is_healthy() {
                return Ok(instance);
            } else {
                tracing::warn!("Unhealthy browser detected, creating new instance");
            }
        }

        // Check if we should scale up the pool
        let current = *self.current_size.read();
        let available = self.pool.len();
        let usage_ratio = 1.0 - (available as f32 / current as f32);

        if usage_ratio >= SCALE_UP_THRESHOLD && current < self.max_size {
            let new_size = (current + 1).min(self.max_size);
            tracing::info!(
                "Scaling up browser pool: {} -> {} (usage: {:.1}%)",
                current,
                new_size,
                usage_ratio * 100.0
            );

            match BrowserInstance::new(&self.launch_options) {
                Ok(new_instance) => {
                    let instance = Arc::new(new_instance);
                    *self.current_size.write() = new_size;
                    return Ok(instance);
                }
                Err(e) => {
                    tracing::error!("Failed to scale up pool: {}", e);
                }
            }
        }

        // Fallback: create temporary instance
        tracing::debug!("Creating temporary browser instance (pool exhausted)");
        BrowserInstance::new(&self.launch_options).map(Arc::new)
    }

    fn release(&self, instance: Arc<BrowserInstance>) {
        if instance.is_healthy() {
            if self.pool.push(instance).is_err() {
                tracing::debug!("Pool full, dropping browser instance");
            }
        } else {
            tracing::warn!("Not returning unhealthy instance to pool");
        }
    }

    fn current_size(&self) -> usize {
        *self.current_size.read()
    }
}

#[derive(Clone)]
pub struct RenderingEngine {
    browser_pool: Arc<BrowserPool>,
    render_semaphore: Arc<Semaphore>,
}

impl RenderingEngine {
    pub fn new() -> Result<Self> {
        Self::with_config(MIN_POOL_SIZE, MAX_POOL_SIZE, MAX_CONCURRENT_RENDERS)
    }

    pub fn with_config(min_pool_size: usize, max_pool_size: usize, max_concurrent: usize) -> Result<Self> {
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

        let browser_pool = BrowserPool::new(min_pool_size, max_pool_size, launch_options)?;
        let render_semaphore = Semaphore::new(max_concurrent);

        Ok(Self {
            browser_pool: Arc::new(browser_pool),
            render_semaphore: Arc::new(render_semaphore),
        })
    }

    pub async fn render(&self, request: RenderRequest) -> Result<Vec<u8>> {
        let _permit = self
            .render_semaphore
            .acquire()
            .await
            .map_err(|_| anyhow!("Failed to acquire render permit"))?;

        tracing::debug!(
            "Render started - Available permits: {}/{}",
            self.render_semaphore.available_permits(),
            MAX_CONCURRENT_RENDERS
        );

        let library_name = request.library.name.clone();
        let format = request.options.format.clone();

        let engine = self.clone();
        let start = Instant::now();

        let result = tokio::task::spawn_blocking(move || engine.render_sync(&request))
            .await
            .map_err(|e| anyhow!("Task join error: {}", e))??;

        let duration = start.elapsed();
        tracing::info!(
            "Render completed in {:?} - Library: {}, Format: {}",
            duration,
            library_name,
            format
        );

        Ok(result)
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

        let browser_instance = self.browser_pool.acquire()?;

        let _pool_guard = BrowserPoolGuard {
            pool: self.browser_pool.clone(),
            instance: Some(browser_instance.clone()),
        };

        let tab = browser_instance.new_tab()?;
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
        let poll_interval =
            Duration::from_millis(request.options.poll_interval_ms.unwrap_or(POLL_INTERVAL_MS));

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

        let render_delay =
            Duration::from_millis(request.options.poll_interval_ms.unwrap_or(POLL_INTERVAL_MS));
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
            "pdf" => tab.print_to_pdf(None)?,
            _ => {
                return Err(anyhow!("Unsupported format: {}", request.options.format));
            }
        };

        Ok(result)
    }

    pub fn health_check(&self) -> HealthStatus {
        HealthStatus {
            pool_size: self.browser_pool.current_size(),
            total_capacity: self.browser_pool.max_size,
            available_permits: self.render_semaphore.available_permits(),
            max_concurrent: MAX_CONCURRENT_RENDERS,
        }
    }
}
