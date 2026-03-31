# DONE: CORS/CSRF Protection

**Completed:** 2026-03-24
**Task:** 2.11 partial (CSRF via CORS restriction)

## What was done
- `rpc/src/server.rs`: Replaced `allow_origin(Any)` with explicit localhost allow-list
- Methods restricted to GET, POST, OPTIONS
- `X-Requested-With` header allowed (CSRF double-submit pattern)
- `build_cors_layer(allowed_origins)` helper function
- `RpcServer::new_with_origins()` for production domain configuration
- Preflight cache reduced to 3600s
