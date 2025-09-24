use crate::models::RenderRequest;
use crate::registry::LIBRARY_REGISTRY;
use anyhow::{Result, anyhow};

pub fn generate_html(request: &RenderRequest) -> Result<String> {
    let library_template = LIBRARY_REGISTRY
        .get(&request.library.name)
        .ok_or_else(|| anyhow!("Unsupported library: {}", request.library.name))?;

    let cdn_url = request
        .library
        .cdn_url
        .clone()
        .unwrap_or_else(|| {
            library_template
                .cdn_url
                .replace("{version}", &request.library.version)
        });

    let data_json = serde_json::to_string(&request.data)?;

    let init_script = library_template
        .init_script
        .replace("{data}", &data_json)
        .replace("{width}", &request.options.width.to_string())
        .replace("{height}", &request.options.height.to_string());

    let canvas_element = if request.library.name == "chartjs" {
        r#"<canvas id="chart-canvas"></canvas>"#
    } else {
        ""
    };

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
        cdn_url,
        init_script
    );

    Ok(html)
}
