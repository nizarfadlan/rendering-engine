use anyhow::{Result, anyhow};
use std::process::{Command, Stdio};
use std::io::Write;

use crate::{core::registry::LIBRARY_REGISTRY, schemas::render::RenderRequest};

/// Generate HTML using Node.js SSR for SolidJS
fn generate_html_nodejs(request: &RenderRequest) -> Result<String> {
    let component_code = request.data.get("componentCode")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'componentCode' in data"))?;

    let default_props = serde_json::json!({});
    let props = request.data.get("props").unwrap_or(&default_props);

    // Prepare input for Node.js script
    let input = serde_json::json!({
        "componentCode": component_code,
        "props": props,
        "width": request.options.width,
        "height": request.options.height
    });

    let input_str = serde_json::to_string(&input)?;

    // Spawn Node.js process
    let mut child = Command::new("node")
        .arg("scripts/solidjs-render.js")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn Node.js process: {}. Make sure Node.js is installed and scripts/solidjs-render.js exists.", e))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_str.as_bytes())
            .map_err(|e| anyhow!("Failed to write to Node.js stdin: {}", e))?;
    }

    // Wait for process and capture output
    let output = child.wait_with_output()
        .map_err(|e| anyhow!("Failed to read Node.js output: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Node.js rendering failed: {}", stderr));
    }

    // Parse output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| anyhow!("Failed to parse Node.js output: {}", e))?;

    if !result.get("success").and_then(|v| v.as_bool()).unwrap_or(false) {
        let error = result.get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(anyhow!("Node.js rendering error: {}", error));
    }

    let html = result.get("html")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'html' in Node.js output"))?;

    Ok(html.to_string())
}

pub fn generate_html(request: &RenderRequest) -> Result<String> {
    // Check if this is Node.js-based rendering
    if request.library.name == "solidjs-node" {
        return generate_html_nodejs(request);
    }
    let library_template = LIBRARY_REGISTRY
        .get(&request.library.name)
        .ok_or_else(|| anyhow!("Unsupported library: {}", request.library.name))?;

    let cdn_url = request.library.cdn_url.clone().unwrap_or_else(|| {
        library_template
            .cdn_url
            .replace("{version}", &request.library.version)
    });

    let data_json = serde_json::to_string(&request.data)?;

    let data_json_escaped = data_json
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");

    let init_script = library_template
        .init_script
        .replace("{data}", &data_json_escaped)
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
        cdn_url,
        init_script
    );

    Ok(html)
}
