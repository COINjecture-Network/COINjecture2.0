// =============================================================================
// Optional TLS support — Phase 2
//
// Reads certificate / key paths from env vars and spawns a TLS-termination
// proxy that accepts HTTPS on the configured external address and forwards
// decrypted TCP traffic to the plain-HTTP jsonrpsee server on localhost.
//
// Env vars:
//   RPC_TLS_CERT   — path to PEM certificate chain
//   RPC_TLS_KEY    — path to PEM private key
//   RPC_TLS_BIND   — external bind address, e.g. "0.0.0.0:8546" (optional;
//                    defaults to same host as backend but port+1)
// =============================================================================

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use tokio::io::AsyncWriteExt;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// TlsConfig
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub bind_addr: SocketAddr,
}

impl TlsConfig {
    /// Construct from environment variables.  Returns `None` when either
    /// `RPC_TLS_CERT` or `RPC_TLS_KEY` is unset.
    pub fn from_env(backend_addr: SocketAddr) -> Option<Self> {
        let cert = std::env::var("RPC_TLS_CERT").ok()?;
        let key = std::env::var("RPC_TLS_KEY").ok()?;

        let bind_addr = std::env::var("RPC_TLS_BIND")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                SocketAddr::new(backend_addr.ip(), backend_addr.port().saturating_add(1))
            });

        Some(TlsConfig {
            cert_path: PathBuf::from(cert),
            key_path: PathBuf::from(key),
            bind_addr,
        })
    }
}

// ---------------------------------------------------------------------------
// Certificate / key loading helpers
// ---------------------------------------------------------------------------

fn load_certs(path: &PathBuf) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read TLS cert {}: {}", path.display(), e))?;
    let mut cursor = std::io::Cursor::new(data);
    certs(&mut cursor)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse TLS cert: {}", e))
}

fn load_key(path: &PathBuf) -> Result<rustls::pki_types::PrivateKeyDer<'static>, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Failed to read TLS key {}: {}", path.display(), e))?;
    let mut cursor = std::io::Cursor::new(data);
    let keys: Vec<_> = pkcs8_private_keys(&mut cursor)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse TLS key (PKCS#8): {}", e))?;
    keys.into_iter()
        .next()
        .map(rustls::pki_types::PrivateKeyDer::Pkcs8)
        .ok_or_else(|| format!("No PKCS#8 private key found in {}", path.display()))
}

// ---------------------------------------------------------------------------
// Build a rustls ServerConfig from cert + key files
// ---------------------------------------------------------------------------

pub fn build_server_config(cfg: &TlsConfig) -> Result<Arc<ServerConfig>, String> {
    let certs = load_certs(&cfg.cert_path)?;
    let key = load_key(&cfg.key_path)?;

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map(Arc::new)
        .map_err(|e| format!("TLS server config error: {}", e))
}

// ---------------------------------------------------------------------------
// TLS termination proxy
//
// Accepts TLS connections on `tls_cfg.bind_addr` and bidirectionally copies
// the decrypted stream to `backend_addr` (the plain-HTTP jsonrpsee server).
// ---------------------------------------------------------------------------

pub async fn run_tls_proxy(
    tls_cfg: TlsConfig,
    backend_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_config = build_server_config(&tls_cfg)
        .map_err(|e| Box::<dyn std::error::Error + Send + Sync>::from(e))?;
    let acceptor = TlsAcceptor::from(server_config);
    let listener = tokio::net::TcpListener::bind(tls_cfg.bind_addr).await?;

    info!(
        tls_addr = %tls_cfg.bind_addr,
        backend = %backend_addr,
        "TLS proxy listening"
    );

    loop {
        let (tcp_stream, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                warn!(error = %e, "TLS accept error");
                continue;
            }
        };

        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(tcp_stream).await {
                Ok(s) => s,
                Err(e) => {
                    warn!(peer = %peer, error = %e, "TLS handshake failed");
                    return;
                }
            };

            let backend = match tokio::net::TcpStream::connect(backend_addr).await {
                Ok(s) => s,
                Err(e) => {
                    error!(backend = %backend_addr, error = %e, "Failed to connect to backend");
                    return;
                }
            };

            let (mut tls_r, mut tls_w) = tokio::io::split(tls_stream);
            let (mut back_r, mut back_w) = tokio::io::split(backend);

            let client_to_backend = tokio::io::copy(&mut tls_r, &mut back_w);
            let backend_to_client = tokio::io::copy(&mut back_r, &mut tls_w);

            tokio::select! {
                _ = client_to_backend => {}
                _ = backend_to_client => {}
            }

            // Best-effort flush / shutdown
            let _ = back_w.shutdown().await;
        });
    }
}
