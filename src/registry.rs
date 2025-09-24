use once_cell::sync::Lazy;
use std::collections::HashMap;

pub struct LibraryTemplate {
    pub cdn_url: String,
    pub wait_selector: String,
    pub init_script: String,
}

pub static LIBRARY_REGISTRY: Lazy<HashMap<String, LibraryTemplate>> = Lazy::new(|| {
    let mut registry = HashMap::new();

    // ECharts
    registry.insert(
        "apache-echarts".to_string(),
        LibraryTemplate {
            cdn_url: "https://cdn.jsdelivr.net/npm/echarts@{version}/dist/echarts.min.js".to_string(),
            wait_selector: "#render-container".to_string(),
            init_script: r#"
                const chart = echarts.init(document.getElementById('render-container'));
                chart.setOption({data});
                window.renderReady = true;
            "#.to_string(),
        },
    );

    // Chart.js
    registry.insert(
        "chartjs".to_string(),
        LibraryTemplate {
            cdn_url: "https://cdn.jsdelivr.net/npm/chart.js@{version}/dist/chart.umd.js".to_string(),
            wait_selector: "#chart-canvas".to_string(),
            init_script: r#"
                const ctx = document.getElementById('chart-canvas').getContext('2d');
                new Chart(ctx, {data});
                window.renderReady = true;
            "#.to_string(),
        },
    );

    // Konva.js
    registry.insert(
        "konvajs".to_string(),
        LibraryTemplate {
            cdn_url: "https://unpkg.com/konva@{version}/konva.min.js".to_string(),
            wait_selector: "#render-container".to_string(),
            init_script: r#"
                const stage = new Konva.Stage({
                    container: 'render-container',
                    width: {width},
                    height: {height}
                });
                const layer = new Konva.Layer();
                stage.add(layer);

                const config = {data};
                if (config.shapes) {
                    config.shapes.forEach(shape => {
                        const konvaShape = new Konva[shape.type](shape.config);
                        layer.add(konvaShape);
                    });
                }

                layer.draw();
                window.renderReady = true;
            "#.to_string(),
        },
    );

    registry.insert(
        "konvajs-json".to_string(),
        LibraryTemplate {
            cdn_url: "https://unpkg.com/konva@{version}/konva.min.js".to_string(),
            wait_selector: "#render-container".to_string(),
            init_script: r#"
                const container = document.getElementById('render-container');
                const konvaJson = {data};

                // Konva.Node.create() akan parse JSON dan create Stage + semua children
                const stage = Konva.Node.create(konvaJson, container);

                // Ensure proper dimensions
                stage.width({width});
                stage.height({height});

                window.renderReady = true;
            "#.to_string(),
        },
    );
    // Add more libraries as needed

    registry
});
