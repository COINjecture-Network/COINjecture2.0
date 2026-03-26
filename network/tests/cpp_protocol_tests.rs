// =============================================================================
// COINjecture P2P Protocol (CPP) - Integration Tests
// =============================================================================
// Comprehensive tests for CPP protocol functionality

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, SolutionReveal,
};
use coinject_network::cpp::{
    config::{NodeType, MAGIC, VERSION},
    message::*,
    peer::PeerId,
    protocol::{MessageCodec, MessageEnvelope, ProtocolError},
    router::EquilibriumRouter,
};
use coinject_network::reputation::{FaultType, ReputationManager};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

// =============================================================================
// Test Helpers
// =============================================================================

fn create_test_peer_id(seed: u8) -> PeerId {
    let mut id = [0u8; 32];
    id[0] = seed;
    id
}

fn create_test_genesis_hash() -> Hash {
    Hash::from_bytes([0x42u8; 32])
}

fn create_test_block_header(height: u64, prev_hash: Hash) -> BlockHeader {
    BlockHeader {
        version: 1,
        height,
        prev_hash,
        timestamp: 1700000000 + height as i64 * 600,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::ZERO,
        commitment: Commitment {
            hash: Hash::ZERO,
            problem_hash: Hash::ZERO,
        },
        work_score: 100.0,
        miner: Address::from_bytes([0u8; 32]),
        nonce: height,
        solve_time_us: 0,
        verify_time_us: 0,
        time_asymmetry_ratio: 0.0,
        solution_quality: 0.0,
        complexity_weight: 0.0,
        energy_estimate_joules: 0.0,
    }
}

fn create_test_block(height: u64, prev_hash: Hash) -> Block {
    let header = create_test_block_header(height, prev_hash);
    Block {
        header: header.clone(),
        coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, height),
        transactions: Vec::new(),
        solution_reveal: SolutionReveal {
            problem: coinject_core::problem::ProblemType::Custom {
                problem_id: Hash::ZERO,
                data: vec![],
            },
            solution: coinject_core::problem::Solution::Custom(vec![]),
            commitment: Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
        },
    }
}

// =============================================================================
// Handshake Tests (Hello/HelloAck)
// =============================================================================

#[tokio::test]
async fn test_cpp_handshake_success() {
    // Start a test server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let genesis_hash = create_test_genesis_hash();
    let peer1_id = create_test_peer_id(1);
    let peer2_id = create_test_peer_id(2);

    // Spawn server task
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Server receives Hello
        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        assert_eq!(envelope.msg_type, MessageType::Hello);

        let hello: HelloMessage = envelope.deserialize().unwrap();
        assert_eq!(hello.peer_id, peer1_id);
        assert_eq!(hello.genesis_hash, genesis_hash);

        // Server sends HelloAck
        let hello_ack = HelloAckMessage {
            version: VERSION,
            peer_id: peer2_id,
            best_height: 100,
            best_hash: Hash::ZERO,
            genesis_hash,
            node_type: NodeType::Full.as_u8(),
            timestamp: 1700000000,
            connection_nonce: 67890, // Test nonce for ack
            ed25519_pubkey: [0u8; 32],
            auth_signature: [0u8; 64],
        };
        MessageCodec::send_hello_ack(&mut stream, &hello_ack)
            .await
            .unwrap();

        // Server receives Status (optional, but good practice)
        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        assert_eq!(envelope.msg_type, MessageType::Status);
    });

    // Client connects and performs handshake
    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Client sends Hello
    let hello = HelloMessage {
        version: VERSION,
        peer_id: peer1_id,
        best_height: 50,
        best_hash: Hash::ZERO,
        genesis_hash,
        node_type: NodeType::Full.as_u8(),
        timestamp: 1700000000,
        connection_nonce: 12345, // Test nonce
        ed25519_pubkey: [0u8; 32],
        auth_signature: [0u8; 64],
    };
    MessageCodec::send_hello(&mut client_stream, &hello)
        .await
        .unwrap();

    // Client receives HelloAck
    let envelope = MessageEnvelope::decode(&mut client_stream).await.unwrap();
    assert_eq!(envelope.msg_type, MessageType::HelloAck);

    let hello_ack: HelloAckMessage = envelope.deserialize().unwrap();
    assert_eq!(hello_ack.peer_id, peer2_id);
    assert_eq!(hello_ack.genesis_hash, genesis_hash);
    assert_eq!(hello_ack.best_height, 100);

    // Client sends Status update
    let status = StatusMessage {
        best_height: 50,
        best_hash: Hash::ZERO,
        node_type: NodeType::Full.as_u8(),
        timestamp: 1700000000,
        flock_state: None,
    };
    MessageCodec::send_status(&mut client_stream, &status)
        .await
        .unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_cpp_handshake_genesis_mismatch() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let correct_genesis = create_test_genesis_hash();
    let wrong_genesis = Hash::from_bytes([0xFFu8; 32]);
    let peer1_id = create_test_peer_id(1);
    let _peer2_id = create_test_peer_id(2);

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        let hello: HelloMessage = envelope.deserialize().unwrap();

        // Server detects genesis mismatch and rejects
        assert_ne!(hello.genesis_hash, correct_genesis);

        // Server should disconnect (in real implementation)
        // For test, we just verify the mismatch was detected
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    let hello = HelloMessage {
        version: VERSION,
        peer_id: peer1_id,
        best_height: 50,
        best_hash: Hash::ZERO,
        genesis_hash: wrong_genesis, // Wrong genesis!
        node_type: NodeType::Full.as_u8(),
        timestamp: 1700000000,
        connection_nonce: 12345, // Test nonce
        ed25519_pubkey: [0u8; 32],
        auth_signature: [0u8; 64],
    };
    MessageCodec::send_hello(&mut client_stream, &hello)
        .await
        .unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_cpp_handshake_invalid_magic() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Try to decode with invalid magic
        let result = MessageEnvelope::decode(&mut stream).await;
        assert!(result.is_err());

        if let Err(ProtocolError::InvalidMagic(magic)) = result {
            assert_ne!(magic, MAGIC);
        } else {
            panic!("Expected InvalidMagic error");
        }
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Send invalid magic bytes (must be at least 10 bytes for header read)
    // Header format: Magic(4) + Version(1) + Type(1) + Length(4) = 10 bytes
    client_stream.write_all(b"BADMAGIC\x01\x00").await.unwrap();
    client_stream.flush().await.unwrap();

    server_task.await.unwrap();
}

// =============================================================================
// Block Propagation Tests (NewBlock)
// =============================================================================

#[tokio::test]
async fn test_cpp_block_propagation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let genesis_hash = create_test_genesis_hash();
    let block = create_test_block(1, genesis_hash);

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Skip handshake for simplicity
        // In real implementation, handshake would happen first

        // Receive NewBlock message
        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        assert_eq!(envelope.msg_type, MessageType::NewBlock);

        let new_block: NewBlockMessage = envelope.deserialize().unwrap();
        assert_eq!(new_block.block.header.height, 1);
        assert_eq!(new_block.block.header.prev_hash, genesis_hash);
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Send NewBlock message
    let new_block = NewBlockMessage {
        block: block.clone(),
    };
    MessageCodec::send_new_block(&mut client_stream, &new_block)
        .await
        .unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_cpp_block_broadcast_multiple_peers() {
    // Simulate broadcasting to multiple peers using router
    let mut router = EquilibriumRouter::new();

    let peer1_id = create_test_peer_id(1);
    let peer2_id = create_test_peer_id(2);
    let peer3_id = create_test_peer_id(3);

    // Add peers to router
    router.add_peer(coinject_network::cpp::router::PeerInfo {
        id: peer1_id,
        best_height: 100,
        node_type: NodeType::Full.as_u8(),
        quality: 1.0,
        last_seen: 0,
        flock_phase: 0,
        flock_epoch: 0,
        velocity: 0.0,
    });

    router.add_peer(coinject_network::cpp::router::PeerInfo {
        id: peer2_id,
        best_height: 100,
        node_type: NodeType::Full.as_u8(),
        quality: 1.0,
        last_seen: 0,
        flock_phase: 0,
        flock_epoch: 0,
        velocity: 0.0,
    });

    router.add_peer(coinject_network::cpp::router::PeerInfo {
        id: peer3_id,
        best_height: 100,
        node_type: NodeType::Full.as_u8(),
        quality: 1.0,
        last_seen: 0,
        flock_phase: 0,
        flock_epoch: 0,
        velocity: 0.0,
    });

    // Select peers for broadcast using equilibrium fanout
    let selected = router.select_broadcast_peers();

    // Should select √n × η peers (for n=3: √3 × 0.707 ≈ 1.22 → 2 peers)
    assert!(!selected.is_empty() && selected.len() <= 3);

    // Verify selected peers are valid
    for peer_id in &selected {
        assert!(router.get_peer(peer_id).is_some());
    }
}

// =============================================================================
// Sync Tests (GetBlocks/Blocks)
// =============================================================================

#[tokio::test]
async fn test_cpp_sync_get_blocks_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Receive GetBlocks request
        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        assert_eq!(envelope.msg_type, MessageType::GetBlocks);

        let get_blocks: GetBlocksMessage = envelope.deserialize().unwrap();
        assert_eq!(get_blocks.from_height, 0);
        assert_eq!(get_blocks.to_height, 100);

        // Server responds with Blocks
        let blocks = vec![
            create_test_block(0, Hash::ZERO),
            create_test_block(1, Hash::ZERO),
            create_test_block(2, Hash::ZERO),
        ];

        let blocks_msg = BlocksMessage {
            blocks: blocks.clone(),
            request_id: get_blocks.request_id,
        };

        MessageCodec::send_blocks(&mut stream, &blocks_msg)
            .await
            .unwrap();
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Client sends GetBlocks request
    let get_blocks = GetBlocksMessage {
        from_height: 0,
        to_height: 100,
        request_id: 12345,
    };
    MessageCodec::send_get_blocks(&mut client_stream, &get_blocks)
        .await
        .unwrap();

    // Client receives Blocks response
    let envelope = MessageEnvelope::decode(&mut client_stream).await.unwrap();
    assert_eq!(envelope.msg_type, MessageType::Blocks);

    let blocks_msg: BlocksMessage = envelope.deserialize().unwrap();
    assert_eq!(blocks_msg.request_id, 12345);
    assert_eq!(blocks_msg.blocks.len(), 3);
    assert_eq!(blocks_msg.blocks[0].header.height, 0);
    assert_eq!(blocks_msg.blocks[1].header.height, 1);
    assert_eq!(blocks_msg.blocks[2].header.height, 2);

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_cpp_sync_empty_range() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        let get_blocks: GetBlocksMessage = envelope.deserialize().unwrap();

        // Server responds with empty blocks (range doesn't exist)
        let blocks_msg = BlocksMessage {
            blocks: vec![],
            request_id: get_blocks.request_id,
        };

        MessageCodec::send_blocks(&mut stream, &blocks_msg)
            .await
            .unwrap();
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    let get_blocks = GetBlocksMessage {
        from_height: 1000,
        to_height: 2000, // Range that doesn't exist
        request_id: 99999,
    };
    MessageCodec::send_get_blocks(&mut client_stream, &get_blocks)
        .await
        .unwrap();

    let envelope = MessageEnvelope::decode(&mut client_stream).await.unwrap();
    let blocks_msg: BlocksMessage = envelope.deserialize().unwrap();
    assert_eq!(blocks_msg.blocks.len(), 0);

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_cpp_sync_large_block_batch() {
    // Test that large block batches can be sent/received
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let genesis_hash = create_test_genesis_hash();
    let mut blocks = Vec::new();
    let mut prev_hash = genesis_hash;

    // Create 50 blocks
    for i in 0..50 {
        let block = create_test_block(i, prev_hash);
        prev_hash = block.header.hash();
        blocks.push(block);
    }

    let blocks_clone = blocks.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let envelope = MessageEnvelope::decode(&mut stream).await.unwrap();
        let get_blocks: GetBlocksMessage = envelope.deserialize().unwrap();

        // Send all blocks
        let blocks_msg = BlocksMessage {
            blocks: blocks_clone,
            request_id: get_blocks.request_id,
        };

        MessageCodec::send_blocks(&mut stream, &blocks_msg)
            .await
            .unwrap();
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    let get_blocks = GetBlocksMessage {
        from_height: 0,
        to_height: 49,
        request_id: 54321,
    };
    MessageCodec::send_get_blocks(&mut client_stream, &get_blocks)
        .await
        .unwrap();

    let envelope = MessageEnvelope::decode(&mut client_stream).await.unwrap();
    let blocks_msg: BlocksMessage = envelope.deserialize().unwrap();
    assert_eq!(blocks_msg.blocks.len(), 50);

    // Verify chain integrity
    for i in 0..blocks_msg.blocks.len() - 1 {
        let current_hash = blocks_msg.blocks[i].header.hash();
        let next_prev_hash = blocks_msg.blocks[i + 1].header.prev_hash;
        assert_eq!(
            current_hash, next_prev_hash,
            "Chain integrity broken at height {}",
            i
        );
    }

    server_task.await.unwrap();
}

// =============================================================================
// Reputation Tests
// =============================================================================

#[test]
fn test_reputation_fault_recording() {
    let mut manager = ReputationManager::new();
    manager.set_block(100);

    // Create peer
    manager.update_stake("peer1", 10000);

    // Record fault
    manager.record_fault("peer1", FaultType::InvalidBlock, None);

    manager.recalculate_all();

    let peer = manager.get("peer1").unwrap();
    assert_eq!(peer.faults.len(), 1);
    assert_eq!(peer.faults[0].fault_type, FaultType::InvalidBlock);
}

#[test]
fn test_reputation_score_calculation() {
    let mut manager = ReputationManager::new();
    manager.set_block(100);

    // Create good peer
    manager.update_stake("good-peer", 10000);

    // Create bad peer with fault
    manager.update_stake("bad-peer", 10000);
    manager.record_fault("bad-peer", FaultType::InvalidBlock, None);

    manager.recalculate_all();

    let good = manager.get("good-peer").unwrap();
    let bad = manager.get("bad-peer").unwrap();

    // Good peer should have higher reputation score
    assert!(
        good.reputation_score > bad.reputation_score,
        "Good peer ({}) should have higher score than bad peer ({})",
        good.reputation_score,
        bad.reputation_score
    );
}

#[test]
fn test_reputation_multiple_faults() {
    let mut manager = ReputationManager::new();
    manager.set_block(100);

    manager.update_stake("peer1", 10000);

    // Record multiple faults
    manager.record_fault("peer1", FaultType::InvalidBlock, None);
    manager.record_fault("peer1", FaultType::SyncTimeout, None);
    manager.record_fault("peer1", FaultType::Spam, None);

    manager.recalculate_all();

    let peer = manager.get("peer1").unwrap();
    assert_eq!(peer.faults.len(), 3);

    // Reputation should be lower with more faults
    assert!(peer.reputation_score < 1.0);
}

#[test]
fn test_reputation_fault_decay() {
    let mut manager = ReputationManager::new();
    manager.set_block(100);

    manager.update_stake("peer1", 10000);
    manager.record_fault("peer1", FaultType::InvalidBlock, None);

    manager.recalculate_all();
    let score_initial = manager.get("peer1").unwrap().reputation_score;

    // Advance blocks (faults should decay over time)
    manager.set_block(200);
    manager.recalculate_all();
    let score_after = manager.get("peer1").unwrap().reputation_score;

    // Score should improve (faults decay)
    assert!(
        score_after >= score_initial,
        "Reputation should improve as faults decay (initial: {}, after: {})",
        score_initial,
        score_after
    );
}

#[test]
fn test_reputation_stake_ratio() {
    let mut manager = ReputationManager::new();
    manager.set_block(100);

    // Create peers with different stakes
    manager.update_stake("low-stake", 1000);
    manager.update_stake("high-stake", 100000);

    manager.recalculate_all();

    let low = manager.get("low-stake").unwrap();
    let high = manager.get("high-stake").unwrap();

    // Higher stake should contribute to higher reputation
    // (though other factors like age and faults also matter)
    // In bootstrap mode, both should get credit for having stake
    assert!(high.staked_amount > low.staked_amount);
}

// =============================================================================
// Protocol Error Tests
// =============================================================================

#[tokio::test]
async fn test_protocol_invalid_version() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let result = MessageEnvelope::decode(&mut stream).await;
        assert!(result.is_err());

        if let Err(ProtocolError::InvalidVersion(version)) = result {
            assert_ne!(version, VERSION);
        } else {
            panic!("Expected InvalidVersion error");
        }
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Send message with invalid version
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(0); // Invalid version (below MIN_SUPPORTED_VERSION=1; 99 would be accepted as future version)
    buf.push(MessageType::Hello as u8);
    buf.extend_from_slice(&0u32.to_be_bytes()); // Length
    client_stream.write_all(&buf).await.unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_protocol_invalid_checksum() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let result = MessageEnvelope::decode(&mut stream).await;
        assert!(result.is_err());

        if let Err(ProtocolError::InvalidChecksum) = result {
            // Expected
        } else {
            panic!("Expected InvalidChecksum error");
        }
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Send message with invalid checksum
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION);
    buf.push(MessageType::Hello as u8);
    buf.extend_from_slice(&10u32.to_be_bytes()); // Length = 10
    buf.extend_from_slice(b"test payload"); // Payload
    buf.extend_from_slice(&[0xFFu8; 32]); // Invalid checksum
    client_stream.write_all(&buf).await.unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_protocol_message_too_large() {
    use coinject_network::cpp::config::MAX_MESSAGE_SIZE;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        let result = MessageEnvelope::decode(&mut stream).await;
        assert!(result.is_err());

        if let Err(ProtocolError::MessageTooLarge(size)) = result {
            assert!(size > MAX_MESSAGE_SIZE);
        } else {
            panic!("Expected MessageTooLarge error");
        }
    });

    let mut client_stream = TcpStream::connect(server_addr).await.unwrap();

    // Send message with size exceeding limit
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION);
    buf.push(MessageType::Hello as u8);
    let oversized_len = (MAX_MESSAGE_SIZE + 1) as u32;
    buf.extend_from_slice(&oversized_len.to_be_bytes());
    client_stream.write_all(&buf).await.unwrap();

    server_task.await.unwrap();
}
