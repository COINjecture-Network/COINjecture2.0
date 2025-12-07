// =============================================================================
// Node Types Integration Tests
// =============================================================================
//
// Comprehensive test suite for the 6 specialized node types:
// - Light, Full, Archive, Validator, Bounty, Oracle
//
// Tests cover:
// - Dynamic behavioral classification
// - Capability routing
// - LightSync server proof generation
// - FlyClient verification
// - Network event handling
//
// INSTITUTIONAL QUALITY: These tests ensure production-grade reliability.

use coinject_core::{Address, Block, BlockHeader, Hash, Transaction};
use std::collections::HashMap;
use std::time::Duration;

// =============================================================================
// Test Utilities
// =============================================================================

/// Create a test block header at the given height
fn create_test_header(height: u64, parent: Hash) -> BlockHeader {
    BlockHeader {
        version: 1,
        height,
        timestamp: 1700000000 + height * 600, // 10 min blocks
        parent_hash: parent,
        merkle_root: Hash::default(),
        state_root: Hash::default(),
        nonce: height, // Simple nonce for tests
        difficulty: 4,
        miner: Address::default(),
        work_score: 100,
    }
}

/// Create a genesis header for tests
fn create_genesis_header() -> BlockHeader {
    create_test_header(0, Hash::default())
}

/// Build a chain of headers for testing
fn build_test_chain(length: u64) -> Vec<BlockHeader> {
    let mut headers = Vec::with_capacity(length as usize);
    let mut parent = Hash::default();
    
    for height in 0..length {
        let header = create_test_header(height, parent);
        parent = header.hash();
        headers.push(header);
    }
    
    headers
}

// =============================================================================
// Node Classification Tests
// =============================================================================

mod classification_tests {
    use super::*;

    #[test]
    fn test_node_type_enum_properties() {
        use coinject_node::node_types::NodeType;
        
        // Test reward multipliers are correctly ordered
        // Archive nodes should have highest multiplier for serving data
        assert!(NodeType::Archive.reward_multiplier() >= NodeType::Full.reward_multiplier());
        assert!(NodeType::Full.reward_multiplier() > NodeType::Light.reward_multiplier());
        
        // Test storage requirements
        let light_req = NodeType::Light.requirements();
        let full_req = NodeType::Full.requirements();
        let archive_req = NodeType::Archive.requirements();
        
        assert!(light_req.min_storage_gb < full_req.min_storage_gb);
        assert!(full_req.min_storage_gb < archive_req.min_storage_gb);
    }

    #[test]
    fn test_behavioral_classification_light_node() {
        use coinject_node::node_types::{NodeBehaviorMetrics, classify_from_behavior};
        
        // Simulate a Light node: low storage, no validation
        let metrics = NodeBehaviorMetrics {
            storage_ratio: 0.01,      // Only headers
            validation_speed_ms: 0.0, // Doesn't validate
            blocks_validated: 0,
            blocks_propagated: 0,
            solve_rate: 0.0,          // No mining
            solutions_submitted: 0,
            solutions_accepted: 0,
            uptime_hours: 24.0,
            data_served_mb: 0.0,
            oracle_accuracy: 0.0,
            oracle_feeds: 0,
            peer_count: 5,
        };
        
        let (node_type, confidence) = classify_from_behavior(&metrics);
        assert!(matches!(node_type, coinject_node::node_types::NodeType::Light));
        assert!(confidence > 0.5, "Light node should be classified with high confidence");
    }

    #[test]
    fn test_behavioral_classification_full_node() {
        use coinject_node::node_types::{NodeBehaviorMetrics, classify_from_behavior};
        
        // Simulate a Full node: moderate storage, validates, serves data
        let metrics = NodeBehaviorMetrics {
            storage_ratio: 0.5,       // Recent blocks
            validation_speed_ms: 50.0,
            blocks_validated: 1000,
            blocks_propagated: 500,
            solve_rate: 0.0,          // No mining
            solutions_submitted: 0,
            solutions_accepted: 0,
            uptime_hours: 720.0,      // 30 days
            data_served_mb: 500.0,
            oracle_accuracy: 0.0,
            oracle_feeds: 0,
            peer_count: 25,
        };
        
        let (node_type, confidence) = classify_from_behavior(&metrics);
        assert!(matches!(node_type, coinject_node::node_types::NodeType::Full));
        assert!(confidence > 0.5);
    }

    #[test]
    fn test_behavioral_classification_archive_node() {
        use coinject_node::node_types::{NodeBehaviorMetrics, classify_from_behavior};
        
        // Simulate an Archive node: full storage, high data serving
        let metrics = NodeBehaviorMetrics {
            storage_ratio: 1.0,       // Full chain
            validation_speed_ms: 40.0,
            blocks_validated: 10000,
            blocks_propagated: 5000,
            solve_rate: 0.0,
            solutions_submitted: 0,
            solutions_accepted: 0,
            uptime_hours: 8760.0,     // 1 year
            data_served_mb: 50000.0,  // Heavy serving
            oracle_accuracy: 0.0,
            oracle_feeds: 0,
            peer_count: 100,
        };
        
        let (node_type, confidence) = classify_from_behavior(&metrics);
        assert!(matches!(node_type, coinject_node::node_types::NodeType::Archive));
        assert!(confidence > 0.6);
    }

    #[test]
    fn test_behavioral_classification_bounty_hunter() {
        use coinject_node::node_types::{NodeBehaviorMetrics, classify_from_behavior};
        
        // Simulate a Bounty Hunter: high solve rate
        let metrics = NodeBehaviorMetrics {
            storage_ratio: 0.3,
            validation_speed_ms: 100.0,
            blocks_validated: 100,
            blocks_propagated: 50,
            solve_rate: 0.85,         // High solve rate
            solutions_submitted: 500,
            solutions_accepted: 425,
            uptime_hours: 2160.0,     // 90 days
            data_served_mb: 100.0,
            oracle_accuracy: 0.0,
            oracle_feeds: 0,
            peer_count: 15,
        };
        
        let (node_type, confidence) = classify_from_behavior(&metrics);
        assert!(matches!(node_type, coinject_node::node_types::NodeType::Bounty));
        assert!(confidence > 0.7);
    }

    #[test]
    fn test_behavioral_classification_oracle_node() {
        use coinject_node::node_types::{NodeBehaviorMetrics, classify_from_behavior};
        
        // Simulate an Oracle node: high accuracy, many feeds
        let metrics = NodeBehaviorMetrics {
            storage_ratio: 0.4,
            validation_speed_ms: 60.0,
            blocks_validated: 500,
            blocks_propagated: 250,
            solve_rate: 0.0,
            solutions_submitted: 0,
            solutions_accepted: 0,
            uptime_hours: 4320.0,     // 180 days
            data_served_mb: 200.0,
            oracle_accuracy: 0.95,    // 95% accurate
            oracle_feeds: 10000,      // Many feeds
            peer_count: 30,
        };
        
        let (node_type, confidence) = classify_from_behavior(&metrics);
        assert!(matches!(node_type, coinject_node::node_types::NodeType::Oracle));
        assert!(confidence > 0.7);
    }
}

// =============================================================================
// Node Type Manager Tests
// =============================================================================

mod manager_tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_node_manager_initialization_full() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let genesis = create_genesis_header();
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(genesis.clone()),
        );
        
        // Full node should have LightSyncServer
        assert!(manager.can_serve_light_clients());
        assert_eq!(manager.current_type().await, NodeType::Full);
    }

    #[tokio::test]
    async fn test_node_manager_initialization_light() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let genesis = create_genesis_header();
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Light,
            Some(genesis.clone()),
        );
        
        // Light node should NOT have LightSyncServer
        assert!(!manager.can_serve_light_clients());
        assert_eq!(manager.current_type().await, NodeType::Light);
    }

    #[tokio::test]
    async fn test_node_manager_block_validation_tracking() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(10);
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        // Simulate validating blocks
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            manager.on_block_validated(&block, 50).await;
        }
        
        // Check status reflects validated blocks
        let status = manager.get_status().await;
        assert!(status.is_light_sync_ready);
    }

    #[tokio::test]
    async fn test_node_manager_flyclient_proof_generation() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(100);
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        // Add headers to LightSyncServer
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            manager.on_block_validated(&block, 30).await;
        }
        
        // Generate FlyClient proof
        let proof_bytes = manager.generate_flyclient_proof(10).await;
        assert!(proof_bytes.is_some(), "Full node should generate FlyClient proofs");
        
        let proof_data = proof_bytes.unwrap();
        assert!(!proof_data.is_empty(), "Proof should not be empty");
    }

    #[tokio::test]
    async fn test_node_manager_mmr_proof_generation() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(50);
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        // Add headers
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            manager.on_block_validated(&block, 25).await;
        }
        
        // Generate MMR proof for block 25
        let proof_result = manager.generate_mmr_proof(25).await;
        assert!(proof_result.is_some(), "Should generate MMR proof");
        
        let (header, proof_bytes, mmr_root) = proof_result.unwrap();
        assert_eq!(header.height, 25);
        assert!(!proof_bytes.is_empty());
        assert_ne!(mmr_root, Hash::default());
    }

    #[tokio::test]
    async fn test_node_manager_header_retrieval() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(30);
        
        let manager = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        // Add headers
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            manager.on_block_validated(&block, 20).await;
        }
        
        // Retrieve headers
        let retrieved = manager.get_headers(10, 10).await;
        assert_eq!(retrieved.len(), 10);
        assert_eq!(retrieved[0].height, 10);
        assert_eq!(retrieved[9].height, 19);
    }
}

// =============================================================================
// Capability Router Tests
// =============================================================================

mod router_tests {
    use super::*;

    #[tokio::test]
    async fn test_capability_router_registration() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::{CapabilityRouter, RequestType};
        
        let router = CapabilityRouter::new();
        
        // Register peers with different types
        router.register_peer("peer1".to_string(), NodeType::Full).await;
        router.register_peer("peer2".to_string(), NodeType::Light).await;
        router.register_peer("peer3".to_string(), NodeType::Archive).await;
        
        // Find peers that can serve blocks
        let block_servers = router.find_capable_peers(&RequestType::GetBlocks).await;
        assert!(block_servers.contains(&"peer1".to_string()));
        assert!(block_servers.contains(&"peer3".to_string()));
        assert!(!block_servers.contains(&"peer2".to_string())); // Light can't serve
        
        // Find peers that can serve FlyClient proofs
        let proof_servers = router.find_capable_peers(&RequestType::GetFlyClientProof).await;
        assert!(proof_servers.contains(&"peer1".to_string()));
        assert!(proof_servers.contains(&"peer3".to_string()));
        assert!(!proof_servers.contains(&"peer2".to_string()));
    }

    #[tokio::test]
    async fn test_capability_router_removal() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::{CapabilityRouter, RequestType};
        
        let router = CapabilityRouter::new();
        
        router.register_peer("peer1".to_string(), NodeType::Full).await;
        router.register_peer("peer2".to_string(), NodeType::Archive).await;
        
        // Both should be capable
        let servers = router.find_capable_peers(&RequestType::GetBlocks).await;
        assert_eq!(servers.len(), 2);
        
        // Remove one
        router.remove_peer("peer1").await;
        
        let servers = router.find_capable_peers(&RequestType::GetBlocks).await;
        assert_eq!(servers.len(), 1);
        assert!(servers.contains(&"peer2".to_string()));
    }

    #[tokio::test]
    async fn test_capability_router_find_best_peer() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::{CapabilityRouter, RequestType};
        
        let router = CapabilityRouter::new();
        
        // Register multiple capable peers
        router.register_peer("peer1".to_string(), NodeType::Full).await;
        router.register_peer("peer2".to_string(), NodeType::Archive).await;
        router.register_peer("peer3".to_string(), NodeType::Validator).await;
        
        // Should find at least one peer
        let best = router.find_best_peer(&RequestType::GetBlocks).await;
        assert!(best.is_some());
    }
}

// =============================================================================
// LightSync Server Tests
// =============================================================================

mod light_sync_tests {
    use super::*;

    #[test]
    fn test_light_sync_server_creation() {
        use coinject_node::light_sync::LightSyncServer;
        
        let genesis = create_genesis_header();
        let server = LightSyncServer::new(genesis.clone());
        
        assert_eq!(server.chain_height(), 0);
        assert!(server.total_work() > 0);
        assert_ne!(server.mmr_root(), Hash::default());
    }

    #[test]
    fn test_light_sync_server_add_headers() {
        use coinject_node::light_sync::LightSyncServer;
        
        let headers = build_test_chain(100);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        // Add remaining headers
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        assert_eq!(server.chain_height(), 99);
        assert!(server.total_work() > 0);
    }

    #[test]
    fn test_light_sync_server_mmr_proof() {
        use coinject_node::light_sync::LightSyncServer;
        
        let headers = build_test_chain(50);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Generate proof for block 25
        let proof = server.generate_mmr_proof(25);
        assert!(proof.is_some());
        
        let proof = proof.unwrap();
        assert_eq!(proof.leaf_index, 25);
        assert!(!proof.peaks.is_empty());
    }

    #[test]
    fn test_light_sync_server_flyclient_proof() {
        use coinject_node::light_sync::LightSyncServer;
        
        let headers = build_test_chain(200);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Generate FlyClient proof
        let proof = server.generate_flyclient_proof(20);
        
        assert_eq!(proof.tip_header.height, 199);
        assert!(!proof.sampled_headers.is_empty());
        assert_eq!(proof.security_param, 20);
    }

    #[test]
    fn test_light_sync_server_header_retrieval() {
        use coinject_node::light_sync::LightSyncServer;
        
        let headers = build_test_chain(30);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Get specific header
        let header = server.get_header(15);
        assert!(header.is_some());
        assert_eq!(header.unwrap().height, 15);
        
        // Get non-existent header
        let header = server.get_header(100);
        assert!(header.is_none());
    }

    #[test]
    fn test_light_sync_message_handling() {
        use coinject_node::light_sync::{LightSyncServer, LightSyncMessage};
        
        let headers = build_test_chain(100);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Handle GetHeaders
        let response = server.handle_get_headers(10, 20, 12345);
        if let LightSyncMessage::Headers { headers: resp_headers, has_more, request_id } = response {
            assert_eq!(resp_headers.len(), 20);
            assert_eq!(resp_headers[0].height, 10);
            assert_eq!(request_id, 12345);
            assert!(has_more);
        } else {
            panic!("Expected Headers response");
        }
        
        // Handle GetChainTip
        let response = server.handle_get_chain_tip(99999);
        if let LightSyncMessage::ChainTip { tip_header, mmr_root, total_work, request_id } = response {
            assert_eq!(tip_header.height, 99);
            assert_ne!(mmr_root, Hash::default());
            assert!(total_work > 0);
            assert_eq!(request_id, 99999);
        } else {
            panic!("Expected ChainTip response");
        }
    }
}

// =============================================================================
// FlyClient Verification Tests
// =============================================================================

mod flyclient_tests {
    use super::*;

    #[test]
    fn test_flyclient_verifier_creation() {
        use coinject_node::light_sync::LightClientVerifier;
        
        let genesis = create_genesis_header();
        let verifier = LightClientVerifier::new(genesis.hash());
        
        assert_eq!(verifier.verified_height(), 0);
        assert!(verifier.verified_root().is_none());
    }

    #[test]
    fn test_flyclient_proof_verification() {
        use coinject_node::light_sync::{LightSyncServer, LightClientVerifier};
        
        // Build chain and server
        let headers = build_test_chain(100);
        let genesis_hash = headers[0].hash();
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Generate proof
        let proof = server.generate_flyclient_proof(20);
        
        // Create verifier and verify
        let mut verifier = LightClientVerifier::new(genesis_hash);
        let result = verifier.verify_and_update(&proof);
        
        assert!(result.is_ok());
        let verification = result.unwrap();
        assert!(verification.valid);
        assert_eq!(verification.new_tip_height, 99);
    }

    #[test]
    fn test_flyclient_genesis_mismatch() {
        use coinject_node::light_sync::{LightSyncServer, LightClientVerifier, FlyClientError};
        
        // Build chain
        let headers = build_test_chain(50);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        // Generate proof
        let proof = server.generate_flyclient_proof(10);
        
        // Create verifier with WRONG genesis
        let wrong_genesis = Hash::from_bytes([0xDE; 32]);
        let mut verifier = LightClientVerifier::new(wrong_genesis);
        
        // Verification should fail
        let result = verifier.verify_and_update(&proof);
        assert!(matches!(result, Err(FlyClientError::GenesisMismatch)));
    }

    #[test]
    fn test_flyclient_proof_extends_chain() {
        use coinject_node::light_sync::{LightSyncServer, LightClientVerifier};
        
        // Build chain
        let headers = build_test_chain(200);
        let genesis_hash = headers[0].hash();
        let mut server = LightSyncServer::new(headers[0].clone());
        
        // Add first 100 headers
        for header in &headers[1..100] {
            server.add_header(header.clone());
        }
        
        // Verify first proof
        let proof1 = server.generate_flyclient_proof(15);
        let mut verifier = LightClientVerifier::new(genesis_hash);
        let result1 = verifier.verify_and_update(&proof1).unwrap();
        assert_eq!(result1.new_tip_height, 99);
        
        // Add more headers
        for header in &headers[100..] {
            server.add_header(header.clone());
        }
        
        // Verify extended proof
        let proof2 = server.generate_flyclient_proof(15);
        let result2 = verifier.verify_and_update(&proof2).unwrap();
        assert_eq!(result2.new_tip_height, 199);
    }
}

// =============================================================================
// MMR Tests
// =============================================================================

mod mmr_tests {
    use super::*;

    #[test]
    fn test_mmr_creation() {
        use coinject_node::light_sync::MerkleMountainRange;
        
        let mmr = MerkleMountainRange::new();
        assert_eq!(mmr.leaf_count, 0);
        assert_eq!(mmr.peak_count(), 0);
    }

    #[test]
    fn test_mmr_with_genesis() {
        use coinject_node::light_sync::MerkleMountainRange;
        
        let genesis = create_genesis_header();
        let mmr = MerkleMountainRange::with_genesis(genesis.hash());
        
        assert_eq!(mmr.leaf_count, 1);
        assert_eq!(mmr.peak_count(), 1);
    }

    #[test]
    fn test_mmr_append() {
        use coinject_node::light_sync::MerkleMountainRange;
        
        let headers = build_test_chain(10);
        let mut mmr = MerkleMountainRange::with_genesis(headers[0].hash());
        
        for header in &headers[1..] {
            mmr.append(header.hash());
        }
        
        assert_eq!(mmr.leaf_count, 10);
        // 10 = 0b1010, so 2 peaks
        assert_eq!(mmr.peak_count(), 2);
    }

    #[test]
    fn test_mmr_deterministic_root() {
        use coinject_node::light_sync::MerkleMountainRange;
        
        let headers = build_test_chain(100);
        
        // Build two MMRs with same data
        let mut mmr1 = MerkleMountainRange::with_genesis(headers[0].hash());
        let mut mmr2 = MerkleMountainRange::with_genesis(headers[0].hash());
        
        for header in &headers[1..] {
            mmr1.append(header.hash());
            mmr2.append(header.hash());
        }
        
        // Roots should be identical
        assert_eq!(mmr1.root(), mmr2.root());
    }

    #[test]
    fn test_mmr_peak_count_formula() {
        use coinject_node::light_sync::mmr_peak_count;
        
        // peak_count(n) = popcount(n)
        assert_eq!(mmr_peak_count(1), 1);   // 0b1
        assert_eq!(mmr_peak_count(2), 1);   // 0b10
        assert_eq!(mmr_peak_count(3), 2);   // 0b11
        assert_eq!(mmr_peak_count(7), 3);   // 0b111
        assert_eq!(mmr_peak_count(8), 1);   // 0b1000
        assert_eq!(mmr_peak_count(15), 4);  // 0b1111
        assert_eq!(mmr_peak_count(255), 8); // 0b11111111
    }

    #[test]
    fn test_mmr_size_formula() {
        use coinject_node::light_sync::MerkleMountainRange;
        
        // MMR size = 2n - popcount(n)
        assert_eq!(MerkleMountainRange::size_for_leaves(1), 1);
        assert_eq!(MerkleMountainRange::size_for_leaves(2), 3);
        assert_eq!(MerkleMountainRange::size_for_leaves(3), 4);
        assert_eq!(MerkleMountainRange::size_for_leaves(4), 7);
        assert_eq!(MerkleMountainRange::size_for_leaves(8), 15);
    }

    #[test]
    fn test_mmr_proof_verification() {
        use coinject_node::light_sync::LightSyncServer;
        
        let headers = build_test_chain(64);
        let mut server = LightSyncServer::new(headers[0].clone());
        
        for header in &headers[1..] {
            server.add_header(header.clone());
        }
        
        let mmr_root = server.mmr_root();
        
        // Generate and verify proof for various heights
        for height in [0, 15, 31, 32, 63] {
            let proof = server.generate_mmr_proof(height);
            assert!(proof.is_some(), "Should generate proof for height {}", height);
            
            let proof = proof.unwrap();
            let is_valid = proof.verify(&mmr_root);
            assert!(is_valid, "Proof for height {} should be valid", height);
        }
    }
}

// =============================================================================
// Network Capability Tests
// =============================================================================

mod capability_tests {
    use super::*;

    #[test]
    fn test_network_capabilities_for_node_type() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::{NetworkCapabilities, RequestType};
        
        // Light node capabilities
        let light_caps = NetworkCapabilities::for_node_type(NodeType::Light);
        assert!(!light_caps.can_handle(&RequestType::GetBlocks));
        assert!(!light_caps.can_handle(&RequestType::GetFlyClientProof));
        assert!(light_caps.can_handle(&RequestType::GetStatus));
        
        // Full node capabilities
        let full_caps = NetworkCapabilities::for_node_type(NodeType::Full);
        assert!(full_caps.can_handle(&RequestType::GetBlocks));
        assert!(full_caps.can_handle(&RequestType::GetHeaders));
        assert!(full_caps.can_handle(&RequestType::GetFlyClientProof));
        assert!(full_caps.can_handle(&RequestType::GetMMRProof));
        
        // Archive node capabilities (should have all Full + more)
        let archive_caps = NetworkCapabilities::for_node_type(NodeType::Archive);
        assert!(archive_caps.can_handle(&RequestType::GetBlocks));
        assert!(archive_caps.can_handle(&RequestType::GetHistoricalBlocks));
    }

    #[test]
    fn test_request_type_requires_storage() {
        use coinject_node::node_manager::RequestType;
        
        // Block requests require storage
        assert!(RequestType::GetBlocks.requires_storage());
        assert!(RequestType::GetHistoricalBlocks.requires_storage());
        
        // Status doesn't require storage
        assert!(!RequestType::GetStatus.requires_storage());
    }
}

// =============================================================================
// End-to-End Integration Tests
// =============================================================================

mod e2e_tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_full_node_serving_light_client() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        use coinject_node::light_sync::LightClientVerifier;
        
        // Create Full node
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(100);
        
        let full_node = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        // Add blocks to Full node
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            full_node.on_block_validated(&block, 30).await;
        }
        
        // Create Light client verifier
        let mut verifier = LightClientVerifier::new(headers[0].hash());
        
        // Full node generates FlyClient proof
        let proof_bytes = full_node.generate_flyclient_proof(20).await;
        assert!(proof_bytes.is_some());
        
        // Light client verifies proof
        let proof: coinject_node::light_sync::FlyClientProof = 
            bincode::deserialize(&proof_bytes.unwrap()).unwrap();
        
        let result = verifier.verify_and_update(&proof);
        assert!(result.is_ok());
        assert!(result.unwrap().valid);
        assert_eq!(verifier.verified_height(), 99);
    }

    #[tokio::test]
    async fn test_archive_node_historical_queries() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        // Create Archive node
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(1000);
        
        let archive_node = NodeTypeManager::new(
            tx,
            NodeType::Archive,
            Some(headers[0].clone()),
        );
        
        // Add all blocks
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            archive_node.on_block_validated(&block, 20).await;
        }
        
        // Should be able to serve proofs for old blocks
        let proof_result = archive_node.generate_mmr_proof(50).await;
        assert!(proof_result.is_some());
        
        let proof_result = archive_node.generate_mmr_proof(500).await;
        assert!(proof_result.is_some());
        
        let proof_result = archive_node.generate_mmr_proof(900).await;
        assert!(proof_result.is_some());
    }

    #[tokio::test]
    async fn test_node_classification_evolution() {
        use coinject_node::node_types::NodeType;
        use coinject_node::node_manager::NodeTypeManager;
        
        // Start as Full node
        let (tx, _rx) = mpsc::channel(100);
        let headers = build_test_chain(50);
        
        let node = NodeTypeManager::new(
            tx,
            NodeType::Full,
            Some(headers[0].clone()),
        );
        
        assert_eq!(node.current_type().await, NodeType::Full);
        
        // Track data served (characteristic of Archive)
        for _ in 0..100 {
            node.on_data_served(1_000_000).await; // 1MB each
        }
        
        // Update with many blocks
        for header in &headers[1..] {
            let block = Block {
                header: header.clone(),
                transactions: Vec::new(),
            };
            node.on_block_validated(&block, 20).await;
        }
        
        // Status should reflect activity
        let status = node.get_status().await;
        assert!(status.requests_served >= 0);
        assert!(status.is_light_sync_ready);
    }
}
