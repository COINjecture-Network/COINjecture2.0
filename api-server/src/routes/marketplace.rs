use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::matching::types::*;
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

#[derive(Deserialize, Default)]
pub struct DatasetQuery {
    pub slug: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct SolutionSetQuery {
    pub problem_type: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct NewOrderRequest {
    pub pair_id: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price: Option<String>,
    pub quantity: String,
    pub time_in_force: Option<String>,
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

/// `GET /marketplace/pairs`
pub async fn get_pairs(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let pairs = supabase
        .get_trading_pairs()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch pairs: {e}")))?;
    Ok(Json(pairs))
}

/// `GET /marketplace/orders` — order book depth (engine-first, Supabase fallback).
pub async fn get_orders(
    State(state): State<AppState>,
    Query(params): Query<OrderQuery>,
) -> Result<Json<Value>, ApiError> {
    let pair_id = params
        .pair_id
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("pair_id query parameter is required".into()))?;

    // Try in-memory engine first
    if let Some(ref engine) = state.engine {
        if let Ok(Some(depth)) = engine.get_depth(pair_id.to_string(), 20).await {
            return Ok(Json(serde_json::to_value(depth).unwrap_or(json!({}))));
        }
    }

    // Fall back to Supabase
    let supabase = require_supabase(&state)?;
    let orders = supabase
        .get_order_book(pair_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch orders: {e}")))?;
    Ok(Json(orders))
}

/// `POST /marketplace/orders` — route through matching engine (or Supabase fallback).
pub async fn place_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<NewOrderRequest>,
) -> Result<Json<Value>, ApiError> {
    let wallet = auth_user
        .wallet_address
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Wallet address required to place orders".into()))?;

    let side = match req.side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => return Err(ApiError::BadRequest("side must be 'buy' or 'sell'".into())),
    };

    let order_type = match req.order_type.to_lowercase().as_str() {
        "limit" => OrderType::Limit,
        "market" => OrderType::Market,
        _ => return Err(ApiError::BadRequest("type must be 'limit' or 'market'".into())),
    };

    let price = match &req.price {
        Some(p) if order_type == OrderType::Limit => Some(
            p.parse::<Decimal>()
                .map_err(|_| ApiError::BadRequest("Invalid price format".into()))?,
        ),
        _ => None,
    };

    let quantity: Decimal = req
        .quantity
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid quantity format".into()))?;

    if quantity <= Decimal::ZERO {
        return Err(ApiError::BadRequest("Quantity must be positive".into()));
    }

    let tif = match req.time_in_force.as_deref() {
        Some("IOC") => TimeInForce::IOC,
        Some("FOK") => TimeInForce::FOK,
        _ => TimeInForce::GTC,
    };

    // If matching engine is available, route through it
    if let Some(ref engine) = state.engine {
        let order = InternalOrder {
            id: Uuid::new_v4(),
            user_id: auth_user.user_id.clone(),
            wallet_address: wallet.to_string(),
            pair_id: req.pair_id.clone(),
            side,
            order_type,
            price,
            quantity,
            filled_quantity: Decimal::ZERO,
            time_in_force: tif,
            created_at: Utc::now(),
        };

        let result = engine
            .submit_order(order)
            .await
            .map_err(|e| ApiError::Internal(format!("Engine error: {e}")))?;

        return Ok(Json(serde_json::to_value(&result).unwrap_or(json!({}))));
    }

    // Fallback: write directly to Supabase
    let supabase = require_supabase(&state)?;
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
    let result = supabase
        .insert_row("orders", body)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to place order: {e}")))?;
    Ok(Json(result))
}

/// `DELETE /marketplace/orders/{id}`
pub async fn cancel_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // Try engine first
    if let Some(ref engine) = state.engine {
        // We need the pair_id to cancel in the engine. For now, try all known pairs.
        // A production system would have an order→pair index.
        if let Ok(oid) = order_id.parse::<Uuid>() {
            for pair in &["BEANS/USDC", "BEANS/ETH"] {
                if let Ok(Some(_)) = engine.cancel_order(oid, pair.to_string()).await {
                    return Ok(Json(json!({
                        "order_id": order_id,
                        "status": "cancelled",
                    })));
                }
            }
        }
    }

    // Fallback: Supabase
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

/// `GET /marketplace/trades`
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
    let trades = supabase
        .get_recent_trades(pair_id, limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch trades: {e}")))?;
    Ok(Json(trades))
}

/// `GET /marketplace/tasks`
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

/// `POST /marketplace/tasks`
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
    let deadline =
        Utc::now() + chrono::Duration::hours(req.deadline_hours.unwrap_or(720) as i64);
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
    let result = supabase
        .insert_row("pouw_tasks", body)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to create task: {e}")))?;
    Ok(Json(result))
}

/// `GET /marketplace/datasets`
pub async fn get_datasets(
    State(state): State<AppState>,
    Query(params): Query<DatasetQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let mut datasets = supabase
        .get_dataset_catalog()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch dataset catalog: {e}")))?;

    if let Some(slug) = params.slug.as_deref() {
        let filtered = datasets
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter(|row| row["slug"].as_str() == Some(slug))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        datasets = Value::Array(filtered);
    }

    Ok(Json(datasets))
}

/// `GET /marketplace/solution-sets`
pub async fn get_solution_sets(
    State(state): State<AppState>,
    Query(params): Query<SolutionSetQuery>,
) -> Result<Json<Value>, ApiError> {
    let supabase = require_supabase(&state)?;
    let solution_sets = supabase
        .get_solution_sets(
            params.problem_type.as_deref(),
            params.sort_by.as_deref().unwrap_or("work_score"),
            params.sort_order.as_deref().unwrap_or("desc"),
            params.limit.unwrap_or(25),
        )
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch solution sets: {e}")))?;

    Ok(Json(solution_sets))
}

/// `GET /marketplace/engine/stats`
pub async fn engine_stats(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let engine = state
        .engine
        .as_ref()
        .ok_or_else(|| ApiError::ServiceUnavailable("Matching engine not running".into()))?;
    let stats = engine
        .get_stats()
        .await
        .map_err(|e| ApiError::Internal(format!("Engine error: {e}")))?;
    Ok(Json(serde_json::to_value(stats).unwrap_or(json!({}))))
}
