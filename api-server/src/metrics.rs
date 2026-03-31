use axum::extract::State;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::time::Instant;

use crate::AppState;

/// Install the global Prometheus metrics recorder and return a handle for rendering.
pub fn init_metrics() -> metrics_exporter_prometheus::PrometheusHandle {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// `GET /metrics` — render Prometheus text exposition format.
pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.metrics_handle.render()
}

/// Middleware that records per-request counters, histograms, and an active-connections gauge.
pub async fn metrics_middleware(request: axum::extract::Request, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    metrics::gauge!("api_active_connections").increment(1.0);
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    metrics::counter!(
        "api_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status
    )
    .increment(1);

    metrics::histogram!(
        "api_request_duration_seconds",
        "method" => method,
        "path" => path
    )
    .record(duration);

    metrics::gauge!("api_active_connections").decrement(1.0);

    response
}
