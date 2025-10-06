# Renderer Engine
A Rust-based rendering engine that uses a headless browser to render charts and graphics from various JavaScript libraries like ECharts, Chart.js, Konva.js, and D3.js. The engine exposes a RESTful API for rendering requests and serves the rendered images.

## Requirements

- Rust (latest stable version recommended)
- Google Chrome or Chromium installed on your system
- Cargo (comes with Rust installation)

## Setup
1. Clone the repository:
   ```bash
   git clone ...
   cd renderer-engine
   ```

2. Install dependencies:
   ```bash
   cargo build --release
   ```

3. Ensure Chrome/Chromium is installed and accessible in your system's PATH.

If you don't have Chrome/Chromium installed, you can download it from:

### Ubuntu/Debian
```bash
sudo apt-get update
sudo apt-get install chromium-browser
```

### Arch Linux
```bash
sudo pacman -S chromium
```

### Fedora
```bash
sudo dnf install chromium
```

### macOS
```bash
brew install --cask chromium
```

4. Run the application:
   ```bash
   cargo run --release
   ```

5. The server will start on `http://localhost:8000`.

## Usage

- Access the API documentation at `http://localhost:8000/docs`.
- Use the `/render` endpoint to render charts by sending a POST request with the required payload
    to `http://localhost:8000/render`.
- Use the `/libraries` endpoint to list supported charting libraries at `http://localhost:8000/libraries`.

## Example Request
```bash
curl -X POST http://localhost:8000/render \
     -H "Content-Type: application/json" \
     -d '{
           "render_type": "echarts",
           "library": {
               "name": "echarts",
               "version": "5.0.0"
           },
           "data": {
               "title": {
                   "text": "ECharts Entry Example"
               },
               "tooltip": {},
               "xAxis": {
                   "data": ["shirt", "cardign", "chiffon shirt", "pants", "heels", "socks"]
               },
               "yAxis": {},
               "series": [{
                   "name": "Sales",
                   "type": "bar",
                   "data": [5, 20, 36, 10, 10, 20]
               }]
           },
           "options": {
                "width": 800,
                "height": 600,
                "format": "png",
           },

         }' --output chart.png
```

## Supported Libraries
- ECharts
- Chart.js
- Konva.js
