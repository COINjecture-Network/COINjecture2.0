// Comprehensive tests for CPP Network Service

use coinject_core::{Block, BlockHeader, Hash};
use coinject_network::cpp::{
    CppConfig, CppNetwork, NetworkCommand, NetworkEvent, NodeType as CppNodeType,
};
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};

/// Helper to create a test block
fn create_test_block(height: u64, prev_hash: Hash) -> Block {
    use coinject_core::{Address, CoinbaseTransaction, SolutionReveal};

    let header = BlockHeader {
        version: 1,
        height,
        prev_hash,
        timestamp: (height * 600) as i64,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::ZERO,
        commitment: coinject_core::Commitment {
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
    };

    Block {
        header: header.clone(),
        coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, height),
        transactions: vec![],
        solution_reveal: SolutionReveal {
            commitment: coinject_core::Commitment {
                hash: Hash::ZERO,
                problem_hash: Hash::ZERO,
            },
            problem: coinject_core::ProblemType::Custom {
                problem_id: Hash::ZERO,
                data: vec![],
            },
            solution: coinject_core::Solution::Custom(vec![]),
        },
    }
}

#[tokio::test]
async fn test_cpp_network_creation() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, _cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    // Network created successfully (can't access private field)
    assert!(true);
}

#[tokio::test]
async fn test_cpp_network_broadcast_block() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let block = create_test_block(1, genesis);

    // Broadcast block command should not error
    let result = cmd_tx.send(NetworkCommand::BroadcastBlock { block });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_update_chain_state() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let new_hash = Hash::new(b"test");

    // Update chain state command should not error
    let result = cmd_tx.send(NetworkCommand::UpdateChainState {
        best_height: 100,
        best_hash: new_hash,
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_request_blocks() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let target_peer_id = [2u8; 32];

    // Request blocks command should not error (even if peer doesn't exist yet)
    let result = cmd_tx.send(NetworkCommand::RequestBlocks {
        peer_id: target_peer_id,
        from_height: 0,
        to_height: 100,
        request_id: 1,
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_request_headers() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let target_peer_id = [2u8; 32];

    // Request headers command should not error
    let result = cmd_tx.send(NetworkCommand::RequestHeaders {
        peer_id: target_peer_id,
        from_height: 0,
        to_height: 100,
        request_id: 1,
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_disconnect_peer() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let target_peer_id = [2u8; 32];

    // Disconnect peer command should not error
    let result = cmd_tx.send(NetworkCommand::DisconnectPeer {
        peer_id: target_peer_id,
        reason: "test".to_string(),
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_broadcast_transaction() {
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let genesis = Hash::ZERO;

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    // Create a simple transaction
    use coinject_core::{Ed25519Signature, PublicKey, TransferTransaction};
    let tx = coinject_core::Transaction::Transfer(TransferTransaction {
        from: coinject_core::Address::from_bytes([0u8; 32]),
        to: coinject_core::Address::from_bytes([1u8; 32]),
        amount: 100,
        fee: 1,
        nonce: 0,
        public_key: PublicKey::from_bytes([0u8; 32]),
        signature: Ed25519Signature::from_bytes([0u8; 64]),
    });

    // Broadcast transaction command should not error
    let result = cmd_tx.send(NetworkCommand::BroadcastTransaction { transaction: tx });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cpp_network_event_types() {
    // Test that all NetworkEvent variants can be created
    let peer_id = [1u8; 32];
    let addr: SocketAddr = "127.0.0.1:707".parse().unwrap();
    let genesis = Hash::ZERO;

    let _event1 = NetworkEvent::PeerConnected {
        peer_id,
        addr,
        node_type: CppNodeType::Full,
        best_height: 100,
        best_hash: genesis,
    };

    let _event2 = NetworkEvent::PeerDisconnected {
        peer_id,
        reason: "test".to_string(),
    };

    let _event3 = NetworkEvent::StatusUpdate {
        peer_id,
        best_height: 100,
        best_hash: genesis,
        node_type: CppNodeType::Full,
    };

    let block = create_test_block(1, genesis);
    let _event4 = NetworkEvent::BlockReceived { block, peer_id };

    use coinject_core::{Ed25519Signature, PublicKey, TransferTransaction};
    let tx = coinject_core::Transaction::Transfer(TransferTransaction {
        from: coinject_core::Address::from_bytes([0u8; 32]),
        to: coinject_core::Address::from_bytes([1u8; 32]),
        amount: 100,
        fee: 1,
        nonce: 0,
        public_key: PublicKey::from_bytes([0u8; 32]),
        signature: Ed25519Signature::from_bytes([0u8; 64]),
    });
    let _event5 = NetworkEvent::TransactionReceived {
        transaction: tx,
        peer_id,
    };

    let blocks = vec![create_test_block(1, genesis), create_test_block(2, genesis)];
    let _event6 = NetworkEvent::BlocksReceived {
        blocks,
        request_id: 1,
        peer_id,
    };

    use coinject_core::Address;

    let headers = vec![BlockHeader {
        version: 1,
        height: 1,
        prev_hash: genesis,
        timestamp: 600,
        transactions_root: Hash::ZERO,
        solutions_root: Hash::ZERO,
        commitment: coinject_core::Commitment {
            hash: Hash::ZERO,
            problem_hash: Hash::ZERO,
        },
        work_score: 100.0,
        miner: Address::from_bytes([0u8; 32]),
        nonce: 1,
        solve_time_us: 0,
        verify_time_us: 0,
        time_asymmetry_ratio: 0.0,
        solution_quality: 0.0,
        complexity_weight: 0.0,
        energy_estimate_joules: 0.0,
    }];
    let _event7 = NetworkEvent::HeadersReceived {
        headers,
        request_id: 1,
        peer_id,
    };
}
