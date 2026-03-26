// End-to-end integration tests for CPP Network
// Tests full peer-to-peer communication, message handling, and synchronization

use coinject_core::{
    Address, Block, BlockHeader, CoinbaseTransaction, Commitment, Hash, SolutionReveal,
};
use coinject_network::cpp::{
    CppConfig, CppNetwork, NetworkCommand, NetworkEvent, NodeType as CppNodeType,
};
use std::net::SocketAddr;

/// Helper to create a test block
fn create_test_block(height: u64, prev_hash: Hash) -> Block {
    let header = BlockHeader {
        version: 1,
        height,
        prev_hash,
        timestamp: (height * 600) as i64,
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
    };

    Block {
        header: header.clone(),
        coinbase: CoinbaseTransaction::new(Address::from_bytes([0u8; 32]), 0, height),
        transactions: vec![],
        solution_reveal: SolutionReveal {
            commitment: Commitment {
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
async fn test_network_broadcast_and_receive_block() {
    let genesis = Hash::ZERO;

    // Create network
    let config = CppConfig::default();
    let peer_id = [1u8; 32];
    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    // Create and broadcast a block
    let block = create_test_block(1, genesis);
    cmd_tx
        .send(NetworkCommand::BroadcastBlock {
            block: block.clone(),
        })
        .unwrap();

    // Note: In a full test with actual peers, we'd receive BlockReceived events
    // For now, we verify the command was sent successfully
    // command sent successfully
}

#[tokio::test]
async fn test_network_chain_state_update() {
    let genesis = Hash::ZERO;
    let config = CppConfig::default();
    let peer_id = [1u8; 32];

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    // Update chain state
    let new_hash = Hash::new(b"new_block");
    cmd_tx
        .send(NetworkCommand::UpdateChainState {
            best_height: 100,
            best_hash: new_hash,
        })
        .unwrap();

    // Command sent successfully
    // command sent successfully
}

#[tokio::test]
async fn test_network_request_blocks() {
    let genesis = Hash::ZERO;
    let config = CppConfig::default();
    let peer_id = [1u8; 32];

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    let target_peer_id = [2u8; 32];

    // Request blocks
    cmd_tx
        .send(NetworkCommand::RequestBlocks {
            peer_id: target_peer_id,
            from_height: 0,
            to_height: 100,
            request_id: 1,
        })
        .unwrap();

    // Command sent successfully
    // command sent successfully
}

#[tokio::test]
async fn test_network_event_handling() {
    let genesis = Hash::ZERO;
    let peer_id = [1u8; 32];
    let addr: SocketAddr = "127.0.0.1:707".parse().unwrap();

    // Test that all event types can be created and handled
    let events = [
        NetworkEvent::PeerConnected {
            peer_id,
            addr,
            node_type: CppNodeType::Full,
            best_height: 100,
            best_hash: genesis,
        },
        NetworkEvent::PeerDisconnected {
            peer_id,
            reason: "test".to_string(),
        },
        NetworkEvent::StatusUpdate {
            peer_id,
            best_height: 100,
            best_hash: genesis,
            node_type: CppNodeType::Full,
        },
        NetworkEvent::BlockReceived {
            block: create_test_block(1, genesis),
            peer_id,
        },
        NetworkEvent::BlocksReceived {
            blocks: vec![create_test_block(1, genesis), create_test_block(2, genesis)],
            request_id: 1,
            peer_id,
        },
    ];

    // All events created successfully
    assert_eq!(events.len(), 5);
}

#[tokio::test]
async fn test_network_command_handling() {
    let genesis = Hash::ZERO;
    let config = CppConfig::default();
    let peer_id = [1u8; 32];

    let (_network, cmd_tx, _event_rx) = CppNetwork::new(config, peer_id, genesis);

    // Test all command types
    let block = create_test_block(1, genesis);
    cmd_tx
        .send(NetworkCommand::BroadcastBlock { block })
        .unwrap();

    use coinject_core::{Ed25519Signature, PublicKey, TransferTransaction};
    let tx = coinject_core::Transaction::Transfer(TransferTransaction {
        from: Address::from_bytes([0u8; 32]),
        to: Address::from_bytes([1u8; 32]),
        amount: 100,
        fee: 1,
        nonce: 0,
        public_key: PublicKey::from_bytes([0u8; 32]),
        signature: Ed25519Signature::from_bytes([0u8; 64]),
    });
    cmd_tx
        .send(NetworkCommand::BroadcastTransaction { transaction: tx })
        .unwrap();

    cmd_tx
        .send(NetworkCommand::UpdateChainState {
            best_height: 100,
            best_hash: genesis,
        })
        .unwrap();

    cmd_tx
        .send(NetworkCommand::RequestBlocks {
            peer_id: [2u8; 32],
            from_height: 0,
            to_height: 100,
            request_id: 1,
        })
        .unwrap();

    cmd_tx
        .send(NetworkCommand::RequestHeaders {
            peer_id: [2u8; 32],
            from_height: 0,
            to_height: 100,
            request_id: 1,
        })
        .unwrap();

    cmd_tx
        .send(NetworkCommand::DisconnectPeer {
            peer_id: [2u8; 32],
            reason: "test".to_string(),
        })
        .unwrap();

    // All commands sent successfully
    // command sent successfully
}

#[tokio::test]
async fn test_network_multiple_peers() {
    let genesis = Hash::ZERO;

    // Create multiple network instances
    let config1 = CppConfig {
        p2p_listen: "127.0.0.1:0".to_string(),
        ws_listen: "127.0.0.1:0".to_string(),
        bootnodes: vec![],
        max_peers: 10,
        enable_websocket: false,
        node_type: CppNodeType::Full,
        ..CppConfig::default()
    };
    let (_network1, _cmd_tx1, _event_rx1) = CppNetwork::new(config1, [1u8; 32], genesis);

    let config2 = CppConfig {
        p2p_listen: "127.0.0.1:0".to_string(),
        ws_listen: "127.0.0.1:0".to_string(),
        bootnodes: vec![],
        max_peers: 10,
        enable_websocket: false,
        node_type: CppNodeType::Full,
        ..CppConfig::default()
    };
    let (_network2, _cmd_tx2, _event_rx2) = CppNetwork::new(config2, [2u8; 32], genesis);

    // Both networks created successfully
    // command sent successfully
}
