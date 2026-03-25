// COINjecture Metrics HTTP Server
// Serves Prometheus metrics for empirical validation

use crate::metrics;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Start metrics HTTP server
pub async fn start_metrics_server(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(
        "📊 Prometheus metrics server listening on http://{}/metrics",
        addr
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_metrics_request))
                .await
            {
                tracing::error!("Error serving metrics connection: {:?}", err);
            }
        });
    }
}

/// Handle incoming HTTP requests for metrics
async fn handle_metrics_request(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    match req.uri().path() {
        "/metrics" => {
            // Export Prometheus metrics
            match metrics::export() {
                Ok(metrics_text) => Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/plain; version=0.0.4")
                    .body(Full::new(Bytes::from(metrics_text)))
                    .unwrap()),
                Err(e) => {
                    tracing::error!("Failed to export metrics: {}", e);
                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::new(Bytes::from("Failed to export metrics")))
                        .unwrap())
                }
            }
        }
        "/health" => {
            // Health check endpoint
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Full::new(Bytes::from(r#"{"status":"healthy"}"#)))
                .unwrap())
        }
        "/" => {
            // Root endpoint with helpful information
            let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>COINjecture Metrics</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        h1 { color: #2c3e50; }
        .endpoint { background: #f8f9fa; padding: 10px; margin: 10px 0; border-radius: 5px; }
        code { background: #e9ecef; padding: 2px 6px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1>COINjecture WEB4 Metrics Server</h1>
    <p>Empirical validation of dimensional economics: η = λ = 1/√2</p>

    <h2>Available Endpoints:</h2>
    <div class="endpoint">
        <strong>GET /metrics</strong><br>
        Prometheus metrics endpoint<br>
        <code>curl http://localhost:9090/metrics</code>
    </div>
    <div class="endpoint">
        <strong>GET /health</strong><br>
        Health check endpoint<br>
        <code>curl http://localhost:9090/health</code>
    </div>

    <h2>Key Metrics for Dimensional Economics Validation:</h2>
    <ul>
        <li><code>coinject_measured_eta</code> - Empirically measured η from pool decay rates</li>
        <li><code>coinject_measured_lambda</code> - Empirically measured λ from pool coupling</li>
        <li><code>coinject_unit_circle_constraint</code> - Validation of |μ|² = η² + λ² = 1</li>
        <li><code>coinject_damping_coefficient</code> - Critical damping ζ = η/√2</li>
        <li><code>coinject_pool_balance</code> - Balance in each dimensional pool</li>
        <li><code>coinject_dimensional_scale</code> - Dimensional scales D_n = e^(-η·τ_n)</li>
    </ul>
</body>
</html>"#;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/html")
                .body(Full::new(Bytes::from(html)))
                .unwrap())
        }
        _ => {
            // 404 for unknown paths
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("Not Found")))
                .unwrap())
        }
    }
}
