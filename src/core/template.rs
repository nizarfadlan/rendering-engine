use anyhow::{Result, anyhow};
use url::Url;

use crate::{core::registry::LIBRARY_REGISTRY, schemas::render::RenderRequest};

pub fn generate_html(request: &RenderRequest) -> Result<String> {
    let library_template = LIBRARY_REGISTRY
        .get(&request.library.name)
        .ok_or_else(|| anyhow!("Unsupported library: {}", request.library.name))?;

    let cdn_url = if let Some(ref custom_url) = request.library.cdn_url {
        validate_cdn_url(custom_url)?;
        custom_url.clone()
    } else {
        library_template
            .cdn_url
            .replace("{version}", &request.library.version)
    };

    let data_json = serde_json::to_string(&request.data)?;

    let init_script = library_template
        .init_script
        .replace("{data}", "JSON.parse(dataJson)")
        .replace("{width}", &request.options.width.to_string())
        .replace("{height}", &request.options.height.to_string());

    let canvas_element = if request.library.name == "chartjs" {
        r#"<canvas id="chart-canvas"></canvas>"#
    } else {
        ""
    };

    let device_pixel_ratio = request.options.device_scale_factor.unwrap_or(1.0);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Render</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            background: white;
            overflow: hidden;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        #render-container {{
            width: {}px;
            height: {}px;
        }}
        #chart-canvas {{
            display: block;
        }}
    </style>
</head>
<body>
    <div id="render-container">
        {}
    </div>

    <script>
        window.devicePixelRatio = {};
        const dataJson = '{}';
    </script>
    <script src="{}"></script>

    <script>
        window.renderReady = false;
        window.renderError = null;

        window.addEventListener('DOMContentLoaded', () => {{
            try {{
                {}
            }} catch (error) {{
                console.error('Render initialization error:', error);
                window.renderError = error.message;
            }}
        }});
    </script>
</body>
</html>"#,
        request.options.width,
        request.options.height,
        canvas_element,
        device_pixel_ratio,
        data_json.replace('\'', "\\'").replace('\n', "\\n"),
        cdn_url,
        init_script
    );

    Ok(html)
}

fn validate_cdn_url(url: &str) -> Result<()> {
    const ALLOWED_DOMAINS: &[&str] = &["cdn.jsdelivr.net", "unpkg.com", "cdnjs.cloudflare.com"];

    let parsed = Url::parse(url).map_err(|_| anyhow!("Invalid CDN URL format"))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("CDN URL must have a host"))?;

    if !ALLOWED_DOMAINS.iter().any(|&allowed| host == allowed) {
        return Err(anyhow!(
            "CDN domain '{}' not allowed. Allowed domains: {:?}",
            host,
            ALLOWED_DOMAINS
        ));
    }

    if parsed.scheme() != "https" {
        return Err(anyhow!("CDN URL must use HTTPS"));
    }

    Ok(())
}
