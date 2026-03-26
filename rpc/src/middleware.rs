// =============================================================================
// RPC Security Middleware — Phase 2
// Implements: rate limiting, auth, body-size guard, admin IP filter,
//             security headers, and structured audit logging as Tower layers.
// =============================================================================

use std::{
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Instant,
};

use bytes::Bytes;
use dashmap::DashMap;
use http::{header, HeaderValue, Request, Response, StatusCode};
use http_body::Body;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full};
use tower::{Layer, Service};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Shared error / body aliases
// ---------------------------------------------------------------------------

pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
/// Opaque body type used by all rejection responses and the pass-through path.
/// Uses `UnsyncBoxBody` to match jsonrpsee's internal body type (which is also !Sync).
pub type RpcBoxBody = UnsyncBoxBody<Bytes, BoxError>;

// ---------------------------------------------------------------------------
// Build a JSON-RPC-shaped error response
// ---------------------------------------------------------------------------

fn make_rejection(status: StatusCode, message: &'static str) -> Response<RpcBoxBody> {
    let payload = format!(
        r#"{{"jsonrpc":"2.0","error":{{"code":-32001,"message":"{}"}},"id":null}}"#,
        message
    );
    let boxed = Full::new(Bytes::from(payload))
        .map_err(|e| -> BoxError { match e {} })
        .boxed_unsync();

    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        // Security headers on every rejection so they are never absent
        .header("X-Content-Type-Options", "nosniff")
        .header("X-Frame-Options", "DENY")
        .header("X-XSS-Protection", "1; mode=block")
        .header(
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains",
        )
        .header("Content-Security-Policy", "default-src 'none'")
        .body(boxed)
        .unwrap()
}

// ---------------------------------------------------------------------------
// SecurityConfig — read from env vars at construction time
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SecurityConfig {
    /// Max requests per minute per IP (env: RPC_RATE_LIMIT_RPM, default 100)
    pub requests_per_minute: u32,
    /// Whether a valid Bearer token is required (env: RPC_REQUIRE_AUTH)
    pub require_auth: bool,
    /// blake3 hashes of accepted API key strings
    pub api_key_hashes: Vec<[u8; 32]>,
    /// IPs allowed to call admin-prefixed paths (env: RPC_ADMIN_ALLOWED_IPS)
    pub admin_allowed_ips: Vec<IpAddr>,
    /// URL path prefixes that are considered admin-only
    pub admin_paths: Vec<String>,
    /// General max body size in bytes (env: RPC_MAX_BODY_BYTES, default 1 MB)
    pub max_body_bytes: usize,
    /// Max body size for transaction-submit endpoints (env: RPC_MAX_TX_BODY_BYTES, default 256 KB)
    pub max_tx_body_bytes: usize,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        let requests_per_minute = std::env::var("RPC_RATE_LIMIT_RPM")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100u32);

        let require_auth = std::env::var("RPC_REQUIRE_AUTH").as_deref() == Ok("true");

        let api_key_hashes: Vec<[u8; 32]> = std::env::var("RPC_API_KEYS")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|k| *blake3::hash(k.trim().as_bytes()).as_bytes())
            .collect();

        let admin_allowed_ips: Vec<IpAddr> = std::env::var("RPC_ADMIN_ALLOWED_IPS")
            .unwrap_or_else(|_| "127.0.0.1".to_string())
            .split(',')
            .filter_map(|s| IpAddr::from_str(s.trim()).ok())
            .collect();
        let admin_allowed_ips = if admin_allowed_ips.is_empty() {
            vec![IpAddr::from_str("127.0.0.1").unwrap()]
        } else {
            admin_allowed_ips
        };

        let max_body_bytes = std::env::var("RPC_MAX_BODY_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1024 * 1024); // 1 MB

        let max_tx_body_bytes = std::env::var("RPC_MAX_TX_BODY_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256 * 1024); // 256 KB

        SecurityConfig {
            requests_per_minute,
            require_auth,
            api_key_hashes,
            admin_allowed_ips,
            admin_paths: vec!["/admin".to_string()],
            max_body_bytes,
            max_tx_body_bytes,
        }
    }
}

impl SecurityConfig {
    /// Store the blake3 hash of a new plaintext API key.
    pub fn add_api_key(&mut self, plaintext: &str) {
        let hash = *blake3::hash(plaintext.as_bytes()).as_bytes();
        if !self.api_key_hashes.contains(&hash) {
            self.api_key_hashes.push(hash);
        }
    }

    /// Remove a key by plaintext (no-op if not present).
    pub fn remove_api_key(&mut self, plaintext: &str) {
        let hash = *blake3::hash(plaintext.as_bytes()).as_bytes();
        self.api_key_hashes.retain(|h| h != &hash);
    }

    /// Replace the full key set (rotation).  Previous keys are immediately invalid.
    pub fn rotate_api_keys(&mut self, new_keys: &[&str]) {
        self.api_key_hashes = new_keys
            .iter()
            .map(|k| *blake3::hash(k.as_bytes()).as_bytes())
            .collect();
    }

    /// Returns true when the bearer token is acceptable.
    pub fn verify_token(&self, token: &str) -> bool {
        if self.api_key_hashes.is_empty() {
            return true; // No keys configured → open access
        }
        let candidate = *blake3::hash(token.as_bytes()).as_bytes();
        self.api_key_hashes.contains(&candidate)
    }
}

// ---------------------------------------------------------------------------
// Token bucket (per-IP)
// ---------------------------------------------------------------------------

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    refill_rate: f64, // tokens / second
}

impl TokenBucket {
    fn new(max_per_minute: u32) -> Self {
        let max = max_per_minute as f64;
        TokenBucket {
            tokens: max,
            last_refill: Instant::now(),
            max_tokens: max,
            refill_rate: max / 60.0,
        }
    }

    /// Returns true and consumes one token, or false if exhausted.
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers shared by layer/service
// ---------------------------------------------------------------------------

fn extract_client_ip<B>(req: &Request<B>) -> String {
    req.headers()
        .get("X-Forwarded-For")
        .or_else(|| req.headers().get("X-Real-IP"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ---------------------------------------------------------------------------
// SecurityGateLayer
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SecurityGateLayer {
    config: Arc<SecurityConfig>,
    buckets: Arc<DashMap<String, Mutex<TokenBucket>>>,
}

impl SecurityGateLayer {
    pub fn new(config: SecurityConfig) -> Self {
        SecurityGateLayer {
            config: Arc::new(config),
            buckets: Arc::new(DashMap::new()),
        }
    }
}

impl<S> Layer<S> for SecurityGateLayer {
    type Service = SecurityGateService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityGateService {
            inner,
            config: self.config.clone(),
            buckets: self.buckets.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// SecurityGateService
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SecurityGateService<S> {
    inner: S,
    config: Arc<SecurityConfig>,
    buckets: Arc<DashMap<String, Mutex<TokenBucket>>>,
}

impl<S, ReqB, ResB> Service<Request<ReqB>> for SecurityGateService<S>
where
    S: Service<Request<ReqB>, Response = Response<ResB>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError>,
    ReqB: Send + 'static,
    ResB: Body<Data = Bytes> + Send + 'static,
    ResB::Error: Into<BoxError>,
{
    type Response = Response<RpcBoxBody>;
    type Error = BoxError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<ReqB>) -> Self::Future {
        // Extract everything we need from the request before consuming it.
        let ip = extract_client_ip(&req);
        let path = req.uri().path().to_string();
        let auth_value: Option<String> = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        let content_length: Option<usize> = req
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok());

        // --- 1. Admin IP filter (synchronous) ---
        let is_admin = self
            .config
            .admin_paths
            .iter()
            .any(|p| path.starts_with(p.as_str()));
        if is_admin {
            let addr_ok = IpAddr::from_str(&ip)
                .map(|a| self.config.admin_allowed_ips.contains(&a))
                .unwrap_or(false);
            if !addr_ok {
                warn!(ip = %ip, path = %path, "admin_access_denied");
                return Box::pin(std::future::ready(Ok(make_rejection(
                    StatusCode::FORBIDDEN,
                    "Forbidden",
                ))));
            }
        }

        // --- 2. Rate limiting (synchronous) ---
        let rpm_ok = {
            let entry = self
                .buckets
                .entry(ip.clone())
                .or_insert_with(|| Mutex::new(TokenBucket::new(self.config.requests_per_minute)));
            entry
                .value()
                .lock()
                .map(|mut b| b.try_consume())
                .unwrap_or(false)
        };
        if !rpm_ok {
            warn!(ip = %ip, "rate_limit_exceeded");
            return Box::pin(std::future::ready(Ok(make_rejection(
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded",
            ))));
        }

        // --- 3. Auth (synchronous) ---
        if self.config.require_auth {
            let token = auth_value
                .as_deref()
                .and_then(|s| s.strip_prefix("Bearer "))
                .unwrap_or("");
            if !self.config.verify_token(token) {
                warn!(ip = %ip, "auth_failed");
                return Box::pin(std::future::ready(Ok(make_rejection(
                    StatusCode::UNAUTHORIZED,
                    "Unauthorized",
                ))));
            }
        }

        // --- 4. Body size check (synchronous via Content-Length) ---
        if let Some(len) = content_length {
            let limit = if path.contains("transaction_submit") {
                self.config.max_tx_body_bytes
            } else {
                self.config.max_body_bytes
            };
            if len > limit {
                warn!(ip = %ip, len, limit, "body_too_large");
                return Box::pin(std::future::ready(Ok(make_rejection(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Request body too large",
                ))));
            }
        }

        // --- Pass through to inner service ---
        let mut inner = self.inner.clone();
        Box::pin(async move {
            let resp = inner.call(req).await.map_err(Into::into)?;
            let (mut parts, body) = resp.into_parts();

            // --- 5. Security headers on every successful response ---
            let h = &mut parts.headers;
            h.insert(
                "X-Content-Type-Options",
                HeaderValue::from_static("nosniff"),
            );
            h.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
            h.insert(
                "X-XSS-Protection",
                HeaderValue::from_static("1; mode=block"),
            );
            h.insert(
                "Strict-Transport-Security",
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            );
            h.insert(
                "Content-Security-Policy",
                HeaderValue::from_static("default-src 'none'"),
            );

            let boxed = body.map_err(Into::into).boxed_unsync();
            Ok(Response::from_parts(parts, boxed))
        })
    }
}

// ---------------------------------------------------------------------------
// AuditLogLayer  — body-type-preserving; just logs metadata
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AuditLogLayer;

impl<S> Layer<S> for AuditLogLayer {
    type Service = AuditLogService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuditLogService { inner }
    }
}

#[derive(Clone)]
pub struct AuditLogService<S> {
    inner: S,
}

impl<S, ReqB, ResB> Service<Request<ReqB>> for AuditLogService<S>
where
    S: Service<Request<ReqB>, Response = Response<ResB>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqB: Send + 'static,
    ResB: Send + 'static,
{
    type Response = Response<ResB>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqB>) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let ip = extract_client_ip(&req);
        let has_auth = req.headers().contains_key(header::AUTHORIZATION);
        let start = Instant::now();

        let fut = self.inner.call(req);

        Box::pin(async move {
            let result: Result<Response<ResB>, S::Error> = fut.await;
            let ms = start.elapsed().as_millis();

            match &result {
                Ok(resp) => {
                    info!(
                        method = %method,
                        path = %path,
                        client_ip = %ip,
                        authenticated = %has_auth,
                        status = resp.status().as_u16(),
                        latency_ms = ms,
                        "rpc.request"
                    );
                }
                Err(_) => {
                    warn!(
                        method = %method,
                        path = %path,
                        client_ip = %ip,
                        latency_ms = ms,
                        "rpc.request.error"
                    );
                }
            }
            result
        })
    }
}
