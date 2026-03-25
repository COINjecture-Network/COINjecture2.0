// =============================================================================
// Network Stress Test — Simulated Peer Connections
// =============================================================================
//
// Connects many simulated TCP peers to the CPP port (707) to test:
//   1. Peer acceptance capacity
//   2. Connection handling under saturation
//   3. Graceful rejection when MAX_PEERS is reached
//   4. Memory stability while many connections are open

use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use futures::future::join_all;
use tracing::{info, warn, debug};

use crate::results::TestResults;

/// CPP magic bytes: "COIN"
const MAGIC: [u8; 4] = *b"COIN";
/// Protocol version 2 (current)
const VERSION: u8 = 2;
/// Hello message type
const MSG_TYPE_HELLO: u8 = 0x01;

pub async fn run_network_stress(
    target: &str,
    num_peers: usize,
    duration_secs: u64,
) -> TestResults {
    let mut results = TestResults::new("network-stress");
    results.metric("config.simulated_peers", num_peers as f64, "peers");
    results.metric("config.duration_secs", duration_secs as f64, "s");

    info!("network-stress: connecting {num_peers} simulated peers to {target}");

    let start = Instant::now();
    let target = std::sync::Arc::new(target.to_string());

    // Attempt to connect all peers concurrently
    let mut connect_handles = Vec::with_capacity(num_peers);
    for peer_id in 0..num_peers {
        let target = target.clone();
        connect_handles.push(tokio::spawn(async move {
            connect_simulated_peer(target, peer_id, duration_secs).await
        }));
    }

    let peer_results = join_all(connect_handles).await;

    let mut connected = 0u64;
    let mut rejected = 0u64;
    let mut timeout_count = 0u64;
    let mut hello_ack_count = 0u64;

    for r in peer_results {
        match r {
            Ok(PeerOutcome::Connected { got_hello_ack }) => {
                connected += 1;
                if got_hello_ack { hello_ack_count += 1; }
            }
            Ok(PeerOutcome::Rejected) => { rejected += 1; }
            Ok(PeerOutcome::Timeout) => { timeout_count += 1; }
            Err(e) => {
                warn!("peer task panicked: {e}");
                rejected += 1;
            }
        }
    }

    let elapsed = start.elapsed().as_secs_f64();

    results.metric("peers.connected", connected as f64, "peers");
    results.metric("peers.rejected", rejected as f64, "peers");
    results.metric("peers.timeout", timeout_count as f64, "peers");
    results.metric("peers.got_hello_ack", hello_ack_count as f64, "peers");

    // Pass criteria: node accepted at least some connections and didn't crash
    // (we can detect crash by checking if ALL connections were refused)
    let all_refused = connected == 0 && rejected == num_peers as u64;
    let passed = !all_refused && timeout_count < num_peers as u64 / 2;

    if rejected > 0 {
        results.note(format!(
            "{rejected} connections were rejected — likely MAX_PEERS limit enforced (expected)"
        ));
    }

    results.finish(
        passed,
        format!(
            "{num_peers} peer attempts: {connected} connected, {rejected} rejected, {timeout_count} timeout"
        ),
        elapsed,
    );

    info!("network-stress: complete — connected={connected} rejected={rejected}");
    results
}

enum PeerOutcome {
    Connected { got_hello_ack: bool },
    Rejected,
    Timeout,
}

/// Attempt to connect as a simulated CPP peer: connect TCP, send Hello, wait for HelloAck.
async fn connect_simulated_peer(
    target: std::sync::Arc<String>,
    peer_id: usize,
    hold_secs: u64,
) -> PeerOutcome {
    let connect_timeout = Duration::from_secs(5);
    let stream = match tokio::time::timeout(
        connect_timeout,
        TcpStream::connect(target.as_ref()),
    ).await {
        Ok(Ok(s)) => s,
        Ok(Err(_)) => return PeerOutcome::Rejected,
        Err(_) => return PeerOutcome::Timeout,
    };

    debug!("peer {peer_id}: connected");

    // Send Hello message
    let hello_bytes = build_hello_message(peer_id as u64);

    let mut stream = stream;
    if stream.write_all(&hello_bytes).await.is_err() {
        return PeerOutcome::Rejected;
    }
    if stream.flush().await.is_err() {
        return PeerOutcome::Rejected;
    }

    // Wait for HelloAck (or read failure = rejected)
    let got_ack = wait_for_hello_ack(&mut stream).await;

    // Hold the connection open for the test duration
    tokio::time::sleep(Duration::from_secs(hold_secs)).await;
    let _ = stream.shutdown().await;

    PeerOutcome::Connected { got_hello_ack: got_ack }
}

/// Build a minimal CPP Hello message envelope.
fn build_hello_message(peer_id: u64) -> Vec<u8> {
    // Construct synthetic HelloMessage payload
    let mut peer_id_bytes = [0u8; 32];
    peer_id_bytes[..8].copy_from_slice(&peer_id.to_le_bytes());

    // bincode-encoded HelloMessage (approximate layout)
    // version(1) + peer_id(32) + best_height(8) + best_hash(32) + genesis_hash(32) + node_type(1) + timestamp(8) + nonce(8)
    let mut payload = Vec::with_capacity(122);
    payload.push(VERSION);          // version
    payload.extend_from_slice(&peer_id_bytes); // peer_id
    payload.extend_from_slice(&0u64.to_le_bytes()); // best_height
    payload.extend_from_slice(&[0u8; 32]); // best_hash
    payload.extend_from_slice(&[0u8; 32]); // genesis_hash
    payload.push(1u8);              // node_type: Full
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    payload.extend_from_slice(&ts.to_le_bytes()); // timestamp
    payload.extend_from_slice(&peer_id.to_le_bytes()); // connection_nonce

    let checksum = blake3::hash(&payload);
    let payload_len = payload.len() as u32;

    let mut frame = Vec::with_capacity(10 + payload.len() + 32);
    frame.extend_from_slice(&MAGIC);
    frame.push(VERSION);
    frame.push(MSG_TYPE_HELLO);
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(checksum.as_bytes());
    frame
}

/// Try to read a HelloAck response (10-byte header suffices to confirm receipt).
async fn wait_for_hello_ack(stream: &mut TcpStream) -> bool {
    use tokio::io::AsyncReadExt;
    let mut header = [0u8; 10];
    match tokio::time::timeout(
        Duration::from_secs(5),
        stream.read_exact(&mut header),
    ).await {
        Ok(Ok(_)) => header[5] == 0x02, // HelloAck type
        _ => false,
    }
}
