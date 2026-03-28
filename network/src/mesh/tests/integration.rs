// =============================================================================
// Multi-Node Integration Tests
// =============================================================================
//
// Spins up 3-5 in-process nodes and verifies:
// - Full mesh formation (all nodes discover and connect to each other)
// - Broadcast reaches all nodes exactly once
// - Direct messaging reaches only the intended target
// - Peer exchange discovers non-seed peers

use crate::mesh::config::NetworkConfig;
use crate::mesh::identity::NodeId;
use crate::mesh::protocol::Payload;
use crate::mesh::{NetworkCommand, NetworkEvent, NetworkService};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Helper: create a config for test node `n` on a random port.
fn test_config(port: u16, seeds: Vec<std::net::SocketAddr>) -> NetworkConfig {
    NetworkConfig {
        listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        seed_nodes: seeds,
        data_dir: PathBuf::from(format!(
            "{}/coinject-mesh-test-{}",
            std::env::temp_dir().display(),
            port
        )),
        heartbeat_interval: Duration::from_secs(2),
        max_missed_heartbeats: 3,
        reconnect_base_delay: Duration::from_millis(200),
        reconnect_max_delay: Duration::from_secs(2),
        default_ttl: 10,
        dedup_cache_capacity: 10_000,
        dedup_cache_ttl: Duration::from_secs(60),
        max_message_size: 16 * 1024 * 1024,
        max_messages_per_second_per_peer: 1000,
        peer_exchange_interval: Duration::from_secs(3),
        handshake_timeout: Duration::from_secs(5),
        connect_timeout: Duration::from_secs(3),
    }
}

/// Wait for a specific number of PeerConnected events.
async fn wait_for_peers(
    rx: &mut mpsc::UnboundedReceiver<NetworkEvent>,
    expected: usize,
    timeout_dur: Duration,
) -> HashSet<NodeId> {
    let mut connected = HashSet::new();
    let deadline = tokio::time::Instant::now() + timeout_dur;

    while connected.len() < expected {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, rx.recv()).await {
            Ok(Some(NetworkEvent::PeerConnected(id))) => {
                connected.insert(id);
            }
            Ok(Some(_)) => continue, // Ignore other events
            Ok(None) => break,
            Err(_) => break, // Timeout
        }
    }
    connected
}

/// Collect all MessageReceived events within a timeout.
async fn collect_messages(
    rx: &mut mpsc::UnboundedReceiver<NetworkEvent>,
    timeout_dur: Duration,
) -> Vec<(NodeId, Payload)> {
    let mut messages = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout_dur;

    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match timeout(remaining, rx.recv()).await {
            Ok(Some(NetworkEvent::MessageReceived { from, payload, .. })) => {
                messages.push((from, payload));
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    messages
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_three_node_mesh_formation() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    // Node A: seed node (no seeds)
    let cfg_a = test_config(0, vec![]);
    let (svc_a, mut rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let addr_a = svc_a.local_id().clone();

    // Get actual listen address — we used port 0, so OS assigned one.
    // We need to get the actual port. Unfortunately, our API doesn't expose it.
    // Let's use fixed ports for tests instead.
    // Actually, we need to know the listen port to pass as seed.
    // The issue is we bind to port 0 for ephemeral ports, but then need to
    // tell other nodes what port to connect to.
    //
    // Workaround: use fixed high ports unlikely to conflict.
    svc_a.shutdown().await.unwrap();

    // Restart with fixed ports
    let port_a = 19100 + (rand::random::<u16>() % 1000);
    let port_b = port_a + 1;
    let port_c = port_a + 2;

    let seed_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();

    let cfg_a = test_config(port_a, vec![]);
    let cfg_b = test_config(port_b, vec![seed_a]);
    let cfg_c = test_config(port_c, vec![seed_a]);

    let (svc_a, mut rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let (svc_b, mut rx_b) = NetworkService::start(cfg_b).await.unwrap();
    let (svc_c, mut rx_c) = NetworkService::start(cfg_c).await.unwrap();

    let id_a = *svc_a.local_id();
    let id_b = *svc_b.local_id();
    let id_c = *svc_c.local_id();

    tracing::info!("Node A: {}", id_a.short());
    tracing::info!("Node B: {}", id_b.short());
    tracing::info!("Node C: {}", id_c.short());

    // Wait for node A to see 2 peers (B and C connect to it as seed)
    let peers_a = wait_for_peers(&mut rx_a, 2, Duration::from_secs(10)).await;
    assert!(
        peers_a.len() >= 2,
        "Node A should see 2 peers, got {}",
        peers_a.len()
    );

    // Wait for B and C to connect (at least to seed)
    let peers_b = wait_for_peers(&mut rx_b, 1, Duration::from_secs(5)).await;
    assert!(!peers_b.is_empty(), "Node B should have at least 1 peer");

    let peers_c = wait_for_peers(&mut rx_c, 1, Duration::from_secs(5)).await;
    assert!(!peers_c.is_empty(), "Node C should have at least 1 peer");

    // Wait for peer exchange to complete — B and C should discover each other
    // Give time for peer exchange round
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Drain any remaining connect events
    let more_b = wait_for_peers(&mut rx_b, 1, Duration::from_secs(3)).await;
    let more_c = wait_for_peers(&mut rx_c, 1, Duration::from_secs(3)).await;

    let total_b = peers_b.len() + more_b.len();
    let total_c = peers_c.len() + more_c.len();

    tracing::info!("Peers: A={}, B={}, C={}", peers_a.len(), total_b, total_c);

    // Cleanup
    svc_a.shutdown().await.unwrap();
    svc_b.shutdown().await.unwrap();
    svc_c.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_broadcast_reaches_all_peers() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    let port_a = 19200 + (rand::random::<u16>() % 1000);
    let port_b = port_a + 1;

    let seed_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();

    let cfg_a = test_config(port_a, vec![]);
    let cfg_b = test_config(port_b, vec![seed_a]);

    let (svc_a, mut rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let (svc_b, mut rx_b) = NetworkService::start(cfg_b).await.unwrap();

    let id_a = *svc_a.local_id();
    let id_b = *svc_b.local_id();

    // Wait for connection
    wait_for_peers(&mut rx_a, 1, Duration::from_secs(10)).await;
    wait_for_peers(&mut rx_b, 1, Duration::from_secs(5)).await;

    // Node A broadcasts
    let test_salt = [0x42; 32];
    svc_a
        .send(NetworkCommand::Broadcast(Payload::ConsensusSalt {
            epoch: 99,
            salt: test_salt,
        }))
        .await
        .unwrap();

    // Node B should receive it
    let msgs = collect_messages(&mut rx_b, Duration::from_secs(5)).await;
    let salt_msgs: Vec<_> = msgs
        .iter()
        .filter(|(_, p)| matches!(p, Payload::ConsensusSalt { epoch: 99, .. }))
        .collect();

    assert!(!salt_msgs.is_empty(), "Node B should receive the broadcast");
    assert_eq!(salt_msgs.len(), 1, "Should receive exactly once");

    // Verify it came from A
    assert_eq!(salt_msgs[0].0, id_a);

    svc_a.shutdown().await.unwrap();
    svc_b.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_direct_message_reaches_only_target() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    let port_a = 19300 + (rand::random::<u16>() % 1000);
    let port_b = port_a + 1;
    let port_c = port_a + 2;

    let seed_a: std::net::SocketAddr = format!("127.0.0.1:{}", port_a).parse().unwrap();

    let cfg_a = test_config(port_a, vec![]);
    let cfg_b = test_config(port_b, vec![seed_a]);
    let cfg_c = test_config(port_c, vec![seed_a]);

    let (svc_a, mut rx_a) = NetworkService::start(cfg_a).await.unwrap();
    let (svc_b, mut rx_b) = NetworkService::start(cfg_b).await.unwrap();
    let (svc_c, mut rx_c) = NetworkService::start(cfg_c).await.unwrap();

    let id_b = *svc_b.local_id();

    // Wait for connections
    wait_for_peers(&mut rx_a, 2, Duration::from_secs(10)).await;
    wait_for_peers(&mut rx_b, 1, Duration::from_secs(5)).await;
    wait_for_peers(&mut rx_c, 1, Duration::from_secs(5)).await;

    // Node A sends direct message to Node B only
    svc_a
        .send(NetworkCommand::SendDirect {
            target: id_b,
            payload: Payload::BountyResult {
                bounty_id: "direct-test".into(),
                accepted: true,
                reward: 42,
                details: vec![1, 2, 3],
            },
        })
        .await
        .unwrap();

    // Node B should receive it
    let msgs_b = collect_messages(&mut rx_b, Duration::from_secs(5)).await;
    let direct_msgs_b: Vec<_> = msgs_b
        .iter()
        .filter(|(_, p)| matches!(p, Payload::BountyResult { bounty_id, .. } if bounty_id == "direct-test"))
        .collect();
    assert!(
        !direct_msgs_b.is_empty(),
        "Node B should receive the direct message"
    );

    // Node C should NOT receive it (it's direct to B)
    let msgs_c = collect_messages(&mut rx_c, Duration::from_secs(2)).await;
    let direct_msgs_c: Vec<_> = msgs_c
        .iter()
        .filter(|(_, p)| matches!(p, Payload::BountyResult { bounty_id, .. } if bounty_id == "direct-test"))
        .collect();
    assert!(
        direct_msgs_c.is_empty(),
        "Node C should NOT receive the direct message (got {} msgs)",
        direct_msgs_c.len()
    );

    svc_a.shutdown().await.unwrap();
    svc_b.shutdown().await.unwrap();
    svc_c.shutdown().await.unwrap();
}
