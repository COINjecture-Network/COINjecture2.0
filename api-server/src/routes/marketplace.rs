use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::errors::ApiError;
use crate::middleware::jwt_auth::AuthenticatedUser;
use crate::AppState;

fn require_supabase(
    state: &AppState,
) -> Result<&std::sync::Arc<crate::supabase::SupabaseClient>, ApiError> {
    state.supabase.as_ref().ok_or_else(|| {
        ApiError::ServiceUnavailable("Marketplace database not available".into())
    })
}

// ── Query params ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct OrderQuery {
    pub pair_id: Option<String>,
    pub side: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct TradeQuery {
    pub pair_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Deserialize, Default)]
pub struct TaskQuery {
    pub status: Option<String>,
    pub class: Option<String>,
}

#[derive(Deserialize)]
pub struct NewOrderRequest {
    pub pair_id: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price: Option<String>,
    pub quantity: String,
}

#[derive(Deserialize)]
pub struct NewTaskRequest {
    pub title: String,
    pub problem_class: String,
    pub problem_data: Value,
    pub bounty_amount: f64,
    pub min_work_score: Option<f64>,
    pub deadline_hours: Option<u32>,
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// `GET /marketplace/pairs` — list active trading pairs.
pub async fn get_pairs(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let pairs = supabase.get_trading_pairs().await.map_err(|e| {
        ApiError::Internal(format!("Failed to fetch pairs: {e}"))
    })?;
    Ok(Json(pairs))
}

/// `GET /marketplace/orders` — get order book for a pair.
pub async fn get_orders(
    State(state): State<AppState>,
    Query(params): Query<OrderQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let pair_id = params
        .pair_id
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("pair_id query parameter is required".into()))?;

    let orders = supabase.get_order_book(pair_id).await.map_err(|e| {
        ApiError::Internal(format!("Failed to fetch orders: {e}"))
    })?;
    Ok(Json(orders))
}

/// `POST /marketplace/orders` — place a new order (protected).
pub async fn place_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<NewOrderRequest>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;

    let wallet = auth_user
        .wallet_address
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Wallet address required to place orders".into()))?;

    let body = json!({
        "user_id": auth_user.user_id,
        "wallet_address": wallet,
        "pair_id": req.pair_id,
        "side": req.side,
        "type": req.order_type,
        "price": req.price,
        "quantity": req.quantity,
        "status": "pending",
    });

    let result = supabase.insert_row("orders", body).await.map_err(|e| {
        ApiError::Internal(format!("Failed to place order: {e}"))
    })?;

    Ok(Json(result))
}

/// `DELETE /marketplace/orders/:id` — cancel an order (protected).
pub async fn cancel_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    supabase
        .cancel_order(&order_id, &auth_user.user_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to cancel order: {e}")))?;

    Ok(Json(json!({
        "order_id": order_id,
        "status": "cancelled",
    })))
}

/// `GET /marketplace/trades` — recent trade history.
pub async fn get_trades(
    State(state): State<AppState>,
    Query(params): Query<TradeQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let pair_id = params
        .pair_id
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("pair_id query parameter is required".into()))?;
    let limit = params.limit.unwrap_or(50);

    let trades = supabase.get_recent_trades(pair_id, limit).await.map_err(|e| {
        ApiError::Internal(format!("Failed to fetch trades: {e}"))
    })?;
    Ok(Json(trades))
}

/// `GET /marketplace/tasks` — list PoUW tasks.
pub async fn get_tasks(
    State(state): State<AppState>,
    Query(params): Query<TaskQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let tasks = supabase
        .get_open_tasks(params.class.as_deref())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch tasks: {e}")))?;
    Ok(Json(tasks))
}

/// `POST /marketplace/tasks` — submit a PoUW task (protected).
pub async fn create_task(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<NewTaskRequest>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;

    let wallet = auth_user
        .wallet_address
        .as_deref()
        .unwrap_or(&auth_user.user_id);

    let deadline = chrono::Utc::now()
        + chrono::Duration::hours(req.deadline_hours.unwrap_or(720) as i64);

    let body = json!({
        "submitter_user_id": auth_user.user_id,
        "submitter_wallet": wallet,
        "title": req.title,
        "problem_class": req.problem_class,
        "problem_data": req.problem_data,
        "bounty_amount": req.bounty_amount,
        "bounty_token": "BEANS",
        "min_work_score": req.min_work_score.unwrap_or(1.0),
        "status": "draft",
        "deadline": deadline.to_rfc3339(),
    });

    let result = supabase.insert_row("pouw_tasks", body).await.map_err(|e| {
        ApiError::Internal(format!("Failed to create task: {e}"))
    })?;

    Ok(Json(result))
}
