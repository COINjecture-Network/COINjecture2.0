use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::Response;
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

/// `GET /marketplace/datasets/{slug}/download`
pub async fn download_dataset(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let supabase = require_supabase(&state)?;
    let datasets = supabase
        .get_dataset_catalog()
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch dataset catalog: {e}")))?;

    let dataset = datasets
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["slug"].as_str() == Some(slug.as_str()))
                .cloned()
        })
        .ok_or_else(|| ApiError::NotFound(format!("Dataset '{slug}' not found")))?;

    let snapshot = dataset
        .get("latest_snapshot")
        .cloned()
        .unwrap_or(Value::Null);
    let end_height = snapshot
        .get("end_height")
        .and_then(|value| value.as_i64())
        .ok_or_else(|| ApiError::NotFound(format!("Dataset '{slug}' has no ready snapshot to download")))?;

    let rows_path = dataset_rows_path(&slug, end_height)?;
    let rows = supabase
        .postgrest_get_public(&rows_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch dataset rows: {e}")))?;
    let version = snapshot
        .get("version")
        .and_then(|value| value.as_str())
        .unwrap_or("latest");
    let exported_at = Utc::now().to_rfc3339();
    let (filename, content_type, body) =
        render_dataset_export(&slug, version, &exported_at, &dataset, &snapshot, &rows)?;

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(body.into())
        .map_err(|e| ApiError::Internal(format!("Failed to build dataset download response: {e}")))
}

fn dataset_rows_path(slug: &str, end_height: i64) -> Result<String, ApiError> {
    let path = match slug {
        "marketplace-events-by-block" => format!(
            "marketplace_block_events?block_height=eq.{end_height}&select=*&order=tx_index.asc,event_index.asc"
        ),
        "problem-submissions-and-solutions" => format!(
            "marketplace_block_events?block_height=eq.{end_height}&event_type=in.(submit_problem,submit_solution)&select=*&order=tx_index.asc,event_index.asc"
        ),
        "bounty-payout-history" => format!(
            "marketplace_block_events?block_height=eq.{end_height}&event_type=eq.claim_bounty&select=*&order=tx_index.asc,event_index.asc"
        ),
        "trading-and-liquidity-activity" => format!(
            "marketplace_block_events?block_height=eq.{end_height}&event_type=in.(trade,liquidity,pool_swap)&select=*&order=tx_index.asc,event_index.asc"
        ),
        "verified-solution-sets" => format!(
            "solution_sets?block_height=eq.{end_height}&select=*&order=created_at.desc"
        ),
        _ => {
            return Err(ApiError::NotFound(format!(
                "Dataset '{slug}' is not available for download"
            )))
        }
    };

    Ok(path)
}

fn render_dataset_export(
    slug: &str,
    version: &str,
    exported_at: &str,
    dataset: &Value,
    snapshot: &Value,
    rows: &Value,
) -> Result<(String, &'static str, Vec<u8>), ApiError> {
    match slug {
        "verified-solution-sets" => {
            let lines = rows
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(serde_json::to_string)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()
                .map_err(|e| ApiError::Internal(format!("Failed to serialize JSONL export: {e}")))?
                .unwrap_or_default();
            let mut body = lines.join("\n");
            if !body.is_empty() {
                body.push('\n');
            }
            Ok((
                format!("{slug}-{version}.jsonl"),
                "application/x-ndjson",
                body.into_bytes(),
            ))
        }
        _ => {
            let csv = rows_to_csv(rows).map_err(ApiError::Internal)?;
            let metadata = format!(
                "# dataset_slug={slug}\n# version={version}\n# exported_at={exported_at}\n# title={}\n# row_count={}\n# checksum={}\n",
                csv_meta_value(dataset.get("title")),
                csv_meta_value(snapshot.get("row_count")),
                csv_meta_value(snapshot.get("checksum"))
            );
            Ok((
                format!("{slug}-{version}.csv"),
                "text/csv; charset=utf-8",
                format!("{metadata}\n{csv}").into_bytes(),
            ))
        }
    }
}

fn rows_to_csv(rows: &Value) -> Result<String, String> {
    let items = rows
        .as_array()
        .ok_or_else(|| "Dataset rows were not returned as an array".to_string())?;

    let mut headers: Vec<String> = Vec::new();
    for item in items {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                if !headers.iter().any(|existing| existing == key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    if headers.is_empty() {
        return Ok("empty_dataset\n".to_string());
    }

    let mut out = String::new();
    out.push_str(&headers.iter().map(|h| csv_escape(h)).collect::<Vec<_>>().join(","));
    out.push('\n');

    for item in items {
        let obj = item
            .as_object()
            .ok_or_else(|| "Dataset row was not an object".to_string())?;
        let row = headers
            .iter()
            .map(|header| csv_escape(&csv_value(obj.get(header))))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&row);
        out.push('\n');
    }

    Ok(out)
}

fn csv_value(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => serde_json::to_string(other).unwrap_or_else(|_| String::new()),
    }
}

fn csv_meta_value(value: Option<&Value>) -> String {
    csv_value(value).replace('\n', " ")
}

fn csv_escape(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    if escaped.contains(',') || escaped.contains('"') || escaped.contains('\n') || escaped.contains('\r') {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
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
