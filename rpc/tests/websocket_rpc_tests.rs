// Comprehensive tests for WebSocket RPC Service

use coinject_core::{Hash, ProblemType};
use coinject_rpc::websocket::{
    ClientId, MiningWork, RpcCommand, RpcEvent, RpcMessage, WebSocketRpc, WorkQueue,
};
use std::net::SocketAddr;

#[tokio::test]
async fn test_websocket_rpc_creation() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let (_rpc, _cmd_tx, _event_rx) = WebSocketRpc::new(addr);

    // Service should be created successfully
}

#[tokio::test]
async fn test_work_queue_add_and_get() {
    let mut queue = WorkQueue::new();

    let work = MiningWork {
        work_id: 0, // Will be set by add_work
        problem: ProblemType::SubsetSum {
            numbers: vec![1, 2, 3, 4, 5],
            target: 7,
        },
        difficulty: 1.0,
        reward: 100,
        expires_at: chrono::Utc::now().timestamp() + 3600,
    };

    queue.add_work(work);

    let client_id: ClientId = "test_client".to_string();
    let retrieved_work = queue.get_work(&client_id);

    assert!(retrieved_work.is_some());
    let work = retrieved_work.unwrap();
    assert_eq!(work.work_id, 1); // First work gets ID 1
    assert!(queue.verify_assignment(&client_id, 1));
}

#[tokio::test]
async fn test_work_queue_expired_work() {
    let mut queue = WorkQueue::new();

    let expired_work = MiningWork {
        work_id: 0,
        problem: ProblemType::SubsetSum {
            numbers: vec![1, 2, 3],
            target: 6,
        },
        difficulty: 1.0,
        reward: 50,
        expires_at: chrono::Utc::now().timestamp() - 100, // Expired
    };

    queue.add_work(expired_work);

    let client_id: ClientId = "test_client".to_string();
    let retrieved_work = queue.get_work(&client_id);

    // Expired work should be filtered out
    assert!(retrieved_work.is_none());
}

#[tokio::test]
async fn test_work_queue_verify_assignment() {
    let mut queue = WorkQueue::new();

    let work = MiningWork {
        work_id: 0,
        problem: ProblemType::SubsetSum {
            numbers: vec![1, 2, 3],
            target: 6,
        },
        difficulty: 1.0,
        reward: 50,
        expires_at: chrono::Utc::now().timestamp() + 3600,
    };

    queue.add_work(work);

    let client_id: ClientId = "test_client".to_string();
    let work = queue.get_work(&client_id).unwrap();

    // Verify correct assignment
    assert!(queue.verify_assignment(&client_id, work.work_id));

    // Verify wrong assignment fails
    assert!(!queue.verify_assignment(&client_id, 999));

    // Verify other client can't use same work ID
    let other_client: ClientId = "other_client".to_string();
    assert!(!queue.verify_assignment(&other_client, work.work_id));
}

#[tokio::test]
async fn test_rpc_message_serialization() {
    // Test that RPC messages can be serialized/deserialized
    let auth_msg = RpcMessage::Auth {
        client_id: "test".to_string(),
        signature: vec![1, 2, 3],
    };

    let json = serde_json::to_string(&auth_msg).unwrap();
    let deserialized: RpcMessage = serde_json::from_str(&json).unwrap();

    match deserialized {
        RpcMessage::Auth {
            client_id,
            signature,
        } => {
            assert_eq!(client_id, "test");
            assert_eq!(signature, vec![1, 2, 3]);
        }
        _ => panic!("Wrong message type"),
    }
}

#[tokio::test]
async fn test_rpc_command_distribute_work() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_rpc, cmd_tx, _event_rx) = WebSocketRpc::new(addr);

    let work = MiningWork {
        work_id: 1,
        problem: ProblemType::SubsetSum {
            numbers: vec![1, 2, 3],
            target: 6,
        },
        difficulty: 1.0,
        reward: 50,
        expires_at: chrono::Utc::now().timestamp() + 3600,
    };

    // Distribute work command should not error
    let result = cmd_tx.send(RpcCommand::DistributeWork { work });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_rpc_command_broadcast_block() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_rpc, cmd_tx, _event_rx) = WebSocketRpc::new(addr);

    let hash = Hash::ZERO;

    // Broadcast block command should not error
    let result = cmd_tx.send(RpcCommand::BroadcastBlock { height: 100, hash });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_rpc_command_notify_reward() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_rpc, cmd_tx, _event_rx) = WebSocketRpc::new(addr);

    let client_id: ClientId = "test_client".to_string();

    // Notify reward command should not error
    let result = cmd_tx.send(RpcCommand::NotifyReward {
        client_id,
        amount: 1000,
        block_height: 100,
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_rpc_command_disconnect_client() {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_rpc, cmd_tx, _event_rx) = WebSocketRpc::new(addr);

    let client_id: ClientId = "test_client".to_string();

    // Disconnect client command should not error
    let result = cmd_tx.send(RpcCommand::DisconnectClient {
        client_id,
        reason: "test".to_string(),
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_rpc_event_types() {
    // Test that all RpcEvent variants can be created
    let client_id: ClientId = "test_client".to_string();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

    let _event1 = RpcEvent::ClientConnected {
        client_id: client_id.clone(),
        addr,
    };

    let _event2 = RpcEvent::ClientDisconnected {
        client_id: client_id.clone(),
    };

    let _event3 = RpcEvent::WorkSubmitted {
        client_id: client_id.clone(),
        work_id: 1,
        solution: vec![1, 2, 3],
        nonce: 12345,
    };

    use coinject_core::PublicKey;
    use coinject_core::{Ed25519Signature, TransferTransaction};
    let tx = coinject_core::Transaction::Transfer(TransferTransaction {
        from: coinject_core::Address::from_bytes([0u8; 32]),
        to: coinject_core::Address::from_bytes([1u8; 32]),
        amount: 100,
        fee: 1,
        nonce: 0,
        public_key: PublicKey::from_bytes([0u8; 32]),
        signature: Ed25519Signature::from_bytes([0u8; 64]),
    });
    let _event4 = RpcEvent::TransactionSubmitted {
        transaction: tx,
        client_id,
    };
}
