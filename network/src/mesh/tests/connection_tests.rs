// =============================================================================
// Connection & Handshake Tests
// =============================================================================

use crate::mesh::connection::{self, ConnectionEvent, ConnectionState, finalize_connection};
use crate::mesh::identity::{Keypair, NodeId};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn test_handshake_bidirectional_auth() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let max_size = 16 * 1024 * 1024;
    let timeout = std::time::Duration::from_secs(5);

    let kp_server = Arc::new(Keypair::generate());
    let kp_client = Arc::new(Keypair::generate());

    let kp_s = kp_server.clone();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        connection::perform_inbound_handshake(&mut stream, &kp_s, addr, max_size, timeout).await
    });

    let kp_c = kp_client.clone();
    let client = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        connection::perform_outbound_handshake(
            &mut stream,
            &kp_c,
            "127.0.0.1:0".parse().unwrap(),
            max_size,
            timeout,
        )
        .await
    });

    let (srv_res, cli_res) = tokio::join!(server, client);
    let (srv_peer_id, _, _) = srv_res.unwrap().unwrap();
    let (cli_peer_id, _, _) = cli_res.unwrap().unwrap();

    assert_eq!(srv_peer_id, *kp_client.node_id());
    assert_eq!(cli_peer_id, *kp_server.node_id());
}

#[tokio::test]
async fn test_finalize_connection_sends_connected_event() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let max_size = 16 * 1024 * 1024;
    let timeout = std::time::Duration::from_secs(5);

    let kp_a = Arc::new(Keypair::generate());
    let kp_b = Arc::new(Keypair::generate());

    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let kp_b_clone = kp_b.clone();
    let conn_tx_clone = conn_tx.clone();
    let shutdown_clone = shutdown_rx.clone();

    // Server: handshake + finalize
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (peer_id, pk, listen) = connection::perform_inbound_handshake(
            &mut stream, &kp_b_clone, addr, max_size, timeout,
        )
        .await
        .unwrap();
        finalize_connection(
            peer_id, listen, pk, stream, conn_tx_clone, max_size, shutdown_clone, false,
        );
    });

    // Client: handshake
    let kp_a_clone = kp_a.clone();
    let client = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        connection::perform_outbound_handshake(
            &mut stream, &kp_a_clone, "127.0.0.1:0".parse().unwrap(), max_size, timeout,
        )
        .await
        .unwrap();
        // Keep stream alive so server read loop doesn't get EOF immediately
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    });

    server.await.unwrap();

    // Should receive a Connected event
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), conn_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        ConnectionEvent::Connected { peer_id, .. } => {
            assert_eq!(peer_id, *kp_a.node_id());
        }
        other => panic!("expected Connected, got {:?}", other),
    }

    client.await.unwrap();
}

#[tokio::test]
async fn test_read_loop_detects_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (conn_tx, mut conn_rx) = mpsc::unbounded_channel();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let peer_id = NodeId([0xAA; 32]);
    let max_size = 16 * 1024 * 1024;

    // Client connects and immediately drops
    let client = tokio::spawn(async move {
        let _stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        // Drop immediately — server should detect EOF
    });

    let (stream, _) = listener.accept().await.unwrap();
    let (reader, _writer) = tokio::io::split(stream);
    connection::spawn_read_loop(peer_id, reader, conn_tx, max_size, shutdown_rx);

    client.await.unwrap();

    // Should get a Disconnected event
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), conn_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        ConnectionEvent::Disconnected { peer_id: pid, .. } => {
            assert_eq!(pid, peer_id);
        }
        other => panic!("expected Disconnected, got {:?}", other),
    }
}

#[test]
fn test_connection_state_transitions() {
    // Just verify the states exist and can be compared
    assert_ne!(ConnectionState::Disconnected, ConnectionState::Connected);
    assert_ne!(ConnectionState::Connecting, ConnectionState::Dead);
    assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
}
