use axum::extract::State;
use axum::response::sse::{KeepAlive, Sse};
use std::time::Duration;

use crate::AppState;

/// `GET /events/blocks` — SSE stream of new block notifications.
pub async fn block_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>
{
    let stream = state.broadcaster.subscribe_blocks();
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

/// `GET /events/mempool` — SSE stream of mempool state changes.
pub async fn mempool_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>
{
    let stream = state.broadcaster.subscribe_mempool();
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

/// `GET /events/marketplace` — SSE stream of marketplace activity.
pub async fn marketplace_stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>
{
    let stream = state.broadcaster.subscribe_marketplace();
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}
