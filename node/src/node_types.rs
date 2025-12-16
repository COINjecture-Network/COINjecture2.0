// =============================================================================
// Specialized Node Types with Dynamic Classification
// =============================================================================
//
// 6 Node Types:
// 1. Light - Header-only sync, minimal storage
// 2. Full - Complete validation, standard storage
// 3. Archive - Complete history, 2TB+ storage
// 4. Validator - Block production, high validation speed
// 5. Bounty - Problem solving focused
// 6. Oracle - External data feeds
//
// CRITICAL: Nodes are classified EMPIRICALLY based on behavior, NOT self-declaration
// This ensures a resilient and meritocratic network structure

use serde::{Deserialize, Serialize};
use std::time::Instant;

// =============================================================================
// Constants
// =============================================================================

/// Storage ratio threshold for Archive nodes (>= 95% of chain)
pub const ARCHIVE_STORAGE_RATIO: f64 = 0.95;

/// Storage ratio threshold for Full nodes (>= 50% of chain)
pub const FULL_STORAGE_RATIO: f64 = 0.50;

/// Storage ratio for Light nodes (< 1% - headers only)
pub const LIGHT_STORAGE_RATIO: f64 = 0.01;

/// Validation speed threshold for Validator nodes (blocks/second)
pub const VALIDATOR_SPEED_THRESHOLD: f64 = 10.0;

/// Solve rate threshold for Bounty nodes (solutions/hour)
pub const BOUNTY_SOLVE_RATE: f64 = 5.0;

/// Oracle uptime requirement (percentage)
pub const ORACLE_UPTIME_THRESHOLD: f64 = 0.99;

/// Minimum observation period for classification (blocks)
pub const MIN_OBSERVATION_BLOCKS: u64 = 1000;

/// Classification update interval (blocks)
pub const CLASSIFICATION_INTERVAL: u64 = 100;

// =============================================================================
// Node Types
// =============================================================================

/// The 6 specialized node types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    /// Header-only sync, minimal storage, mobile-friendly
    Light,
    /// Full chain validation, standard storage
    Full,
    /// Complete history preservation, 2TB+ storage
    Archive,
    /// Block production and validation, high performance
    Validator,
    /// NP-problem solving focused
    Bounty,
    /// External data feeds and cross-chain bridges
    Oracle,
}

impl NodeType {
    /// Get reward multiplier for this node type
    /// Based on the golden ratio cascade: φ, 1/φ, 1/φ², etc.
    pub fn reward_multiplier(&self) -> f64 {
        match self {
            NodeType::Light => 0.146,      // D8 scale - minimal contribution
            NodeType::Full => 0.500,       // D5 scale - standard
            NodeType::Archive => 0.382,    // D6 scale - storage premium
            NodeType::Validator => 1.000,  // D1 scale - highest
            NodeType::Bounty => 0.618,     // D4 scale - golden ratio
            NodeType::Oracle => 0.750,     // D3 scale - data premium
        }
    }

    /// Get minimum stake requirement (in tokens)
    pub fn min_stake(&self) -> u128 {
        match self {
            NodeType::Light => 0,                     // No stake required
            NodeType::Full => 1_000_000_000,          // 1,000 tokens
            NodeType::Archive => 10_000_000_000,      // 10,000 tokens
            NodeType::Validator => 100_000_000_000,   // 100,000 tokens
            NodeType::Bounty => 5_000_000_000,        // 5,000 tokens
            NodeType::Oracle => 50_000_000_000,       // 50,000 tokens
        }
    }

    /// Get minimum hardware requirements
    pub fn hardware_requirements(&self) -> HardwareRequirements {
        match self {
            NodeType::Light => HardwareRequirements {
                min_ram_gb: 1,
                min_storage_gb: 10,
                min_bandwidth_mbps: 10,
                min_cpu_cores: 1,
            },
            NodeType::Full => HardwareRequirements {
                min_ram_gb: 8,
                min_storage_gb: 500,
                min_bandwidth_mbps: 100,
                min_cpu_cores: 4,
            },
            NodeType::Archive => HardwareRequirements {
                min_ram_gb: 32,
                min_storage_gb: 2000, // 2TB
                min_bandwidth_mbps: 1000,
                min_cpu_cores: 8,
            },
            NodeType::Validator => HardwareRequirements {
                min_ram_gb: 16,
                min_storage_gb: 500,
                min_bandwidth_mbps: 500,
                min_cpu_cores: 8,
            },
            NodeType::Bounty => HardwareRequirements {
                min_ram_gb: 64,
                min_storage_gb: 100,
                min_bandwidth_mbps: 100,
                min_cpu_cores: 16, // CPU-intensive
            },
            NodeType::Oracle => HardwareRequirements {
                min_ram_gb: 8,
                min_storage_gb: 100,
                min_bandwidth_mbps: 500,
                min_cpu_cores: 4,
            },
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            NodeType::Light => "Header-only sync for mobile/embedded devices",
            NodeType::Full => "Complete validation with standard storage",
            NodeType::Archive => "Full historical data preservation (2TB+)",
            NodeType::Validator => "Block production and high-speed validation",
            NodeType::Bounty => "NP-problem solving and bounty hunting",
            NodeType::Oracle => "External data feeds and cross-chain bridges",
        }
    }

    /// All node types
    pub fn all() -> Vec<NodeType> {
        vec![
            NodeType::Light,
            NodeType::Full,
            NodeType::Archive,
            NodeType::Validator,
            NodeType::Bounty,
            NodeType::Oracle,
        ]
    }
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Light => write!(f, "LIGHT"),
            NodeType::Full => write!(f, "FULL"),
            NodeType::Archive => write!(f, "ARCHIVE"),
            NodeType::Validator => write!(f, "VALIDATOR"),
            NodeType::Bounty => write!(f, "BOUNTY"),
            NodeType::Oracle => write!(f, "ORACLE"),
        }
    }
}

/// Hardware requirements for a node type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRequirements {
    pub min_ram_gb: u32,
    pub min_storage_gb: u32,
    pub min_bandwidth_mbps: u32,
    pub min_cpu_cores: u32,
}

// =============================================================================
// Behavioral Metrics
// =============================================================================

/// Behavioral metrics collected for classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeBehaviorMetrics {
    // === Storage Metrics ===
    /// Blocks stored locally
    pub blocks_stored: u64,
    /// Total chain height (for ratio calculation)
    pub chain_height: u64,
    /// Storage used in bytes
    pub storage_bytes: u64,
    /// Headers stored (for light nodes)
    pub headers_only: bool,
    
    // === Validation Metrics ===
    /// Blocks validated per second (rolling average)
    pub validation_speed: f64,
    /// Total blocks validated
    pub blocks_validated: u64,
    /// Validation errors
    pub validation_errors: u64,
    
    // === Solving Metrics ===
    /// Solutions submitted
    pub solutions_submitted: u64,
    /// Valid solutions (accepted)
    pub solutions_accepted: u64,
    /// Solve rate (solutions per hour)
    pub solve_rate: f64,
    
    // === Uptime Metrics ===
    /// Total uptime in seconds
    pub uptime_seconds: u64,
    /// Total expected uptime
    pub expected_uptime_seconds: u64,
    /// Connection drops
    pub connection_drops: u64,
    
    // === Network Metrics ===
    /// Blocks propagated
    pub blocks_propagated: u64,
    /// Peer count (average)
    pub avg_peer_count: f64,
    /// Data served to peers (bytes)
    pub data_served_bytes: u64,
    
    // === Oracle Metrics ===
    /// External data feeds provided
    pub oracle_feeds_provided: u64,
    /// Oracle data accuracy (0.0 - 1.0)
    pub oracle_accuracy: f64,
    
    // === Timing ===
    /// First observation block
    pub first_observation_block: u64,
    /// Last update block
    pub last_update_block: u64,
    /// Observation started (not serialized)
    #[serde(skip)]
    pub observation_started: Option<Instant>,
}

impl NodeBehaviorMetrics {
    pub fn new(chain_height: u64) -> Self {
        NodeBehaviorMetrics {
            blocks_stored: 0,
            chain_height,
            storage_bytes: 0,
            headers_only: false,
            validation_speed: 0.0,
            blocks_validated: 0,
            validation_errors: 0,
            solutions_submitted: 0,
            solutions_accepted: 0,
            solve_rate: 0.0,
            uptime_seconds: 0,
            expected_uptime_seconds: 0,
            connection_drops: 0,
            blocks_propagated: 0,
            avg_peer_count: 0.0,
            data_served_bytes: 0,
            oracle_feeds_provided: 0,
            oracle_accuracy: 0.0,
            first_observation_block: chain_height,
            last_update_block: chain_height,
            observation_started: Some(Instant::now()),
        }
    }

    /// Calculate storage ratio
    pub fn storage_ratio(&self) -> f64 {
        if self.chain_height == 0 {
            return 0.0;
        }
        self.blocks_stored as f64 / self.chain_height as f64
    }

    /// Calculate uptime ratio
    pub fn uptime_ratio(&self) -> f64 {
        if self.expected_uptime_seconds == 0 {
            return 0.0;
        }
        self.uptime_seconds as f64 / self.expected_uptime_seconds as f64
    }

    /// Calculate validation accuracy
    pub fn validation_accuracy(&self) -> f64 {
        let total = self.blocks_validated + self.validation_errors;
        if total == 0 {
            return 1.0;
        }
        self.blocks_validated as f64 / total as f64
    }

    /// Calculate solution acceptance rate
    pub fn solution_acceptance_rate(&self) -> f64 {
        if self.solutions_submitted == 0 {
            return 0.0;
        }
        self.solutions_accepted as f64 / self.solutions_submitted as f64
    }

    /// Get observation duration in blocks
    pub fn observation_blocks(&self) -> u64 {
        self.last_update_block.saturating_sub(self.first_observation_block)
    }

    /// Check if enough data for classification
    pub fn has_enough_data(&self) -> bool {
        self.observation_blocks() >= MIN_OBSERVATION_BLOCKS
    }
}

// =============================================================================
// Dynamic Classification
// =============================================================================

/// Node classification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Determined node type
    pub node_type: NodeType,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Classification reason
    pub reason: String,
    /// Secondary type (if applicable)
    pub secondary_type: Option<NodeType>,
    /// Block height at classification
    pub classified_at_block: u64,
    /// Scores for each type
    pub type_scores: Vec<(NodeType, f64)>,
}

/// Classify a node based on its behavioral metrics
/// This is the core meritocratic classification logic
pub fn classify_from_behavior(metrics: &NodeBehaviorMetrics) -> ClassificationResult {
    let mut scores: Vec<(NodeType, f64)> = Vec::new();
    
    // Calculate score for each node type
    scores.push((NodeType::Light, score_light(metrics)));
    scores.push((NodeType::Full, score_full(metrics)));
    scores.push((NodeType::Archive, score_archive(metrics)));
    scores.push((NodeType::Validator, score_validator(metrics)));
    scores.push((NodeType::Bounty, score_bounty(metrics)));
    scores.push((NodeType::Oracle, score_oracle(metrics)));
    
    // Sort by score (highest first)
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    let (best_type, best_score) = scores[0];
    let secondary = if scores.len() > 1 && scores[1].1 > 0.3 {
        Some(scores[1].0)
    } else {
        None
    };
    
    // Determine confidence based on gap between top scores
    let confidence = if scores.len() > 1 {
        let gap = best_score - scores[1].1;
        (0.5 + gap).min(1.0)
    } else {
        best_score
    };
    
    let reason = generate_classification_reason(best_type, metrics);
    
    ClassificationResult {
        node_type: best_type,
        confidence,
        reason,
        secondary_type: secondary,
        classified_at_block: metrics.last_update_block,
        type_scores: scores,
    }
}

/// Score for Light node classification
fn score_light(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    
    // Headers only is a strong indicator
    if metrics.headers_only {
        score += 0.5;
    }
    
    // Very low storage ratio
    if metrics.storage_ratio() < LIGHT_STORAGE_RATIO {
        score += 0.3;
    }
    
    // Low validation activity
    if metrics.blocks_validated < 100 {
        score += 0.2;
    }
    
    score.min(1.0)
}

/// Score for Full node classification
fn score_full(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    let storage_ratio = metrics.storage_ratio();
    
    // Good storage ratio (50-95%)
    if storage_ratio >= FULL_STORAGE_RATIO && storage_ratio < ARCHIVE_STORAGE_RATIO {
        score += 0.4;
    }
    
    // Validates blocks but not at validator speed
    if metrics.validation_speed > 0.0 && metrics.validation_speed < VALIDATOR_SPEED_THRESHOLD {
        score += 0.3;
    }
    
    // Good uptime
    if metrics.uptime_ratio() > 0.8 {
        score += 0.2;
    }
    
    // Serves data to peers
    if metrics.data_served_bytes > 0 {
        score += 0.1;
    }
    
    score.min(1.0)
}

/// Score for Archive node classification
fn score_archive(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    
    // Very high storage ratio
    if metrics.storage_ratio() >= ARCHIVE_STORAGE_RATIO {
        score += 0.5;
    }
    
    // Large storage
    let storage_gb = metrics.storage_bytes / (1024 * 1024 * 1024);
    if storage_gb >= 2000 {
        score += 0.3;
    } else if storage_gb >= 1000 {
        score += 0.1;
    }
    
    // Serves lots of data (historical queries)
    if metrics.data_served_bytes > 1_000_000_000_000 { // 1TB served
        score += 0.2;
    }
    
    score.min(1.0)
}

/// Score for Validator node classification
fn score_validator(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    
    // High validation speed
    if metrics.validation_speed >= VALIDATOR_SPEED_THRESHOLD {
        score += 0.4;
    }
    
    // High block propagation
    if metrics.blocks_propagated > 1000 {
        score += 0.2;
    }
    
    // High validation accuracy
    if metrics.validation_accuracy() > 0.99 {
        score += 0.2;
    }
    
    // Good uptime
    if metrics.uptime_ratio() > 0.95 {
        score += 0.1;
    }
    
    // Many peers (well connected)
    if metrics.avg_peer_count > 20.0 {
        score += 0.1;
    }
    
    score.min(1.0)
}

/// Score for Bounty node classification
fn score_bounty(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    
    // High solve rate
    if metrics.solve_rate >= BOUNTY_SOLVE_RATE {
        score += 0.5;
    } else if metrics.solve_rate >= BOUNTY_SOLVE_RATE / 2.0 {
        score += 0.25;
    }
    
    // Good solution acceptance rate
    if metrics.solution_acceptance_rate() > 0.8 {
        score += 0.3;
    }
    
    // Has submitted solutions
    if metrics.solutions_submitted > 10 {
        score += 0.2;
    }
    
    score.min(1.0)
}

/// Score for Oracle node classification
fn score_oracle(metrics: &NodeBehaviorMetrics) -> f64 {
    let mut score = 0.0_f64;
    
    // Provides oracle feeds
    if metrics.oracle_feeds_provided > 0 {
        score += 0.4;
    }
    
    // High accuracy
    if metrics.oracle_accuracy > 0.99 {
        score += 0.3;
    }
    
    // Very high uptime (critical for oracles)
    if metrics.uptime_ratio() >= ORACLE_UPTIME_THRESHOLD {
        score += 0.3;
    }
    
    score.min(1.0)
}

/// Generate human-readable classification reason
fn generate_classification_reason(node_type: NodeType, metrics: &NodeBehaviorMetrics) -> String {
    match node_type {
        NodeType::Light => {
            format!(
                "Headers-only: {}, Storage ratio: {:.2}%",
                metrics.headers_only,
                metrics.storage_ratio() * 100.0
            )
        }
        NodeType::Full => {
            format!(
                "Storage: {:.1}%, Validation: {:.1} blocks/sec",
                metrics.storage_ratio() * 100.0,
                metrics.validation_speed
            )
        }
        NodeType::Archive => {
            let storage_gb = metrics.storage_bytes / (1024 * 1024 * 1024);
            format!(
                "Storage: {:.1}% ({} GB), Data served: {} GB",
                metrics.storage_ratio() * 100.0,
                storage_gb,
                metrics.data_served_bytes / (1024 * 1024 * 1024)
            )
        }
        NodeType::Validator => {
            format!(
                "Validation speed: {:.1} blocks/sec, Accuracy: {:.2}%, Propagated: {}",
                metrics.validation_speed,
                metrics.validation_accuracy() * 100.0,
                metrics.blocks_propagated
            )
        }
        NodeType::Bounty => {
            format!(
                "Solve rate: {:.1}/hr, Acceptance: {:.1}%, Solutions: {}",
                metrics.solve_rate,
                metrics.solution_acceptance_rate() * 100.0,
                metrics.solutions_accepted
            )
        }
        NodeType::Oracle => {
            format!(
                "Feeds provided: {}, Accuracy: {:.2}%, Uptime: {:.2}%",
                metrics.oracle_feeds_provided,
                metrics.oracle_accuracy * 100.0,
                metrics.uptime_ratio() * 100.0
            )
        }
    }
}

// =============================================================================
// Node Classification Manager
// =============================================================================

/// Manager for tracking node classifications
#[derive(Debug)]
pub struct NodeClassificationManager {
    /// Current node's metrics
    pub local_metrics: NodeBehaviorMetrics,
    /// Current classification
    pub current_classification: Option<ClassificationResult>,
    /// Classification history
    pub classification_history: Vec<ClassificationResult>,
    /// Target node type (operator preference)
    pub target_type: Option<NodeType>,
    /// Last classification block
    pub last_classification_block: u64,
}

impl NodeClassificationManager {
    pub fn new(chain_height: u64) -> Self {
        NodeClassificationManager {
            local_metrics: NodeBehaviorMetrics::new(chain_height),
            current_classification: None,
            classification_history: Vec::new(),
            target_type: None,
            last_classification_block: 0,
        }
    }

    /// Set target type (operator's preference)
    pub fn set_target_type(&mut self, node_type: NodeType) {
        self.target_type = Some(node_type);
    }

    /// Update chain height
    pub fn update_chain_height(&mut self, height: u64) {
        self.local_metrics.chain_height = height;
        self.local_metrics.last_update_block = height;
    }

    /// Record block stored
    pub fn record_block_stored(&mut self) {
        self.local_metrics.blocks_stored += 1;
    }

    /// Record block validated
    pub fn record_block_validated(&mut self, duration_ms: u64) {
        self.local_metrics.blocks_validated += 1;
        
        // Update rolling average validation speed
        let speed = if duration_ms > 0 {
            1000.0 / duration_ms as f64
        } else {
            100.0 // Max speed if instant
        };
        
        // EMA with α = 0.1
        self.local_metrics.validation_speed = 
            0.1 * speed + 0.9 * self.local_metrics.validation_speed;
    }

    /// Record validation error
    pub fn record_validation_error(&mut self) {
        self.local_metrics.validation_errors += 1;
    }

    /// Record solution submitted
    pub fn record_solution_submitted(&mut self, accepted: bool) {
        self.local_metrics.solutions_submitted += 1;
        if accepted {
            self.local_metrics.solutions_accepted += 1;
        }
        
        // Update solve rate (solutions per hour)
        if let Some(started) = self.local_metrics.observation_started {
            let hours = started.elapsed().as_secs_f64() / 3600.0;
            if hours > 0.0 {
                self.local_metrics.solve_rate = 
                    self.local_metrics.solutions_accepted as f64 / hours;
            }
        }
    }

    /// Record block propagated
    pub fn record_block_propagated(&mut self) {
        self.local_metrics.blocks_propagated += 1;
    }

    /// Record data served
    pub fn record_data_served(&mut self, bytes: u64) {
        self.local_metrics.data_served_bytes += bytes;
    }

    /// Record oracle feed
    pub fn record_oracle_feed(&mut self, accurate: bool) {
        self.local_metrics.oracle_feeds_provided += 1;
        
        // Update accuracy EMA
        let accuracy_value = if accurate { 1.0 } else { 0.0 };
        self.local_metrics.oracle_accuracy = 
            0.1 * accuracy_value + 0.9 * self.local_metrics.oracle_accuracy;
    }

    /// Update uptime
    pub fn update_uptime(&mut self, uptime_seconds: u64, expected_seconds: u64) {
        self.local_metrics.uptime_seconds = uptime_seconds;
        self.local_metrics.expected_uptime_seconds = expected_seconds;
    }

    /// Record connection drop
    pub fn record_connection_drop(&mut self) {
        self.local_metrics.connection_drops += 1;
    }

    /// Update peer count
    pub fn update_peer_count(&mut self, count: usize) {
        // EMA for average peer count
        self.local_metrics.avg_peer_count = 
            0.1 * count as f64 + 0.9 * self.local_metrics.avg_peer_count;
    }

    /// Set headers-only mode
    pub fn set_headers_only(&mut self, headers_only: bool) {
        self.local_metrics.headers_only = headers_only;
    }

    /// Update storage size
    pub fn update_storage(&mut self, bytes: u64) {
        self.local_metrics.storage_bytes = bytes;
    }

    /// Reclassify if needed
    pub fn maybe_reclassify(&mut self, current_block: u64) -> Option<ClassificationResult> {
        // Check if enough time has passed
        if current_block < self.last_classification_block + CLASSIFICATION_INTERVAL {
            return None;
        }
        
        // Check if we have enough data
        if !self.local_metrics.has_enough_data() {
            return None;
        }
        
        // Perform classification
        let result = classify_from_behavior(&self.local_metrics);
        
        // Store result
        self.current_classification = Some(result.clone());
        self.classification_history.push(result.clone());
        self.last_classification_block = current_block;
        
        // Trim history
        if self.classification_history.len() > 100 {
            self.classification_history.remove(0);
        }
        
        Some(result)
    }

    /// Get current node type
    pub fn current_type(&self) -> NodeType {
        self.current_classification
            .as_ref()
            .map(|c| c.node_type)
            .unwrap_or(NodeType::Full) // Default to Full
    }

    /// Get current reward multiplier
    pub fn reward_multiplier(&self) -> f64 {
        self.current_type().reward_multiplier()
    }

    /// Check if meeting target requirements
    pub fn is_meeting_target(&self) -> Option<(bool, String)> {
        let target = self.target_type?;
        let current = self.current_type();
        
        if current == target {
            Some((true, format!("Meeting {} target", target)))
        } else {
            let advice = self.get_improvement_advice(target);
            Some((false, advice))
        }
    }

    /// Get advice for improving to target type
    fn get_improvement_advice(&self, target: NodeType) -> String {
        let metrics = &self.local_metrics;
        
        match target {
            NodeType::Archive => {
                format!(
                    "Need storage ratio >= 95% (current: {:.1}%). Store more blocks.",
                    metrics.storage_ratio() * 100.0
                )
            }
            NodeType::Validator => {
                format!(
                    "Need validation speed >= {} blocks/sec (current: {:.1}). Upgrade hardware.",
                    VALIDATOR_SPEED_THRESHOLD,
                    metrics.validation_speed
                )
            }
            NodeType::Bounty => {
                format!(
                    "Need solve rate >= {}/hr (current: {:.1}). Focus on problem solving.",
                    BOUNTY_SOLVE_RATE,
                    metrics.solve_rate
                )
            }
            NodeType::Oracle => {
                format!(
                    "Need uptime >= 99% (current: {:.1}%). Improve reliability.",
                    metrics.uptime_ratio() * 100.0
                )
            }
            _ => {
                format!("Continue current operation toward {} type.", target)
            }
        }
    }

    /// Get status summary
    pub fn status(&self) -> NodeTypeStatus {
        NodeTypeStatus {
            current_type: self.current_type(),
            target_type: self.target_type,
            confidence: self.current_classification
                .as_ref()
                .map(|c| c.confidence)
                .unwrap_or(0.0),
            reward_multiplier: self.reward_multiplier(),
            observation_blocks: self.local_metrics.observation_blocks(),
            storage_ratio: self.local_metrics.storage_ratio(),
            validation_speed: self.local_metrics.validation_speed,
            solve_rate: self.local_metrics.solve_rate,
            uptime_ratio: self.local_metrics.uptime_ratio(),
        }
    }
}

/// Node type status summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTypeStatus {
    pub current_type: NodeType,
    pub target_type: Option<NodeType>,
    pub confidence: f64,
    pub reward_multiplier: f64,
    pub observation_blocks: u64,
    pub storage_ratio: f64,
    pub validation_speed: f64,
    pub solve_rate: f64,
    pub uptime_ratio: f64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reward_multipliers() {
        // Validator should have highest reward
        assert_eq!(NodeType::Validator.reward_multiplier(), 1.0);
        
        // Archive should be mid-tier
        assert!(NodeType::Archive.reward_multiplier() > NodeType::Light.reward_multiplier());
        
        // Bounty at golden ratio
        assert!((NodeType::Bounty.reward_multiplier() - 0.618).abs() < 0.001);
    }

    #[test]
    fn test_light_classification() {
        let mut metrics = NodeBehaviorMetrics::new(10000);
        metrics.headers_only = true;
        metrics.blocks_stored = 50; // 0.5% storage
        metrics.last_update_block = 11000;
        
        let result = classify_from_behavior(&metrics);
        assert_eq!(result.node_type, NodeType::Light);
    }

    #[test]
    fn test_archive_classification() {
        let mut metrics = NodeBehaviorMetrics::new(10000);
        metrics.blocks_stored = 9800; // 98% storage
        metrics.storage_bytes = 2 * 1024 * 1024 * 1024 * 1024; // 2TB
        metrics.data_served_bytes = 1_500_000_000_000; // 1.5TB served
        metrics.last_update_block = 11000;
        
        let result = classify_from_behavior(&metrics);
        assert_eq!(result.node_type, NodeType::Archive);
    }

    #[test]
    fn test_validator_classification() {
        let mut metrics = NodeBehaviorMetrics::new(10000);
        metrics.blocks_stored = 7000; // 70% storage (Full level)
        metrics.validation_speed = 15.0; // High speed
        metrics.blocks_validated = 5000;
        metrics.blocks_propagated = 2000;
        metrics.uptime_seconds = 86000;
        metrics.expected_uptime_seconds = 86400;
        metrics.avg_peer_count = 25.0;
        metrics.last_update_block = 11000;
        
        let result = classify_from_behavior(&metrics);
        assert_eq!(result.node_type, NodeType::Validator);
    }

    #[test]
    fn test_bounty_classification() {
        let mut metrics = NodeBehaviorMetrics::new(10000);
        metrics.solve_rate = 10.0; // 10 solutions/hour
        metrics.solutions_submitted = 50;
        metrics.solutions_accepted = 45; // 90% acceptance
        metrics.last_update_block = 11000;
        
        let result = classify_from_behavior(&metrics);
        assert_eq!(result.node_type, NodeType::Bounty);
    }

    #[test]
    fn test_oracle_classification() {
        let mut metrics = NodeBehaviorMetrics::new(10000);
        metrics.oracle_feeds_provided = 100;
        metrics.oracle_accuracy = 0.995;
        metrics.uptime_seconds = 86400;
        metrics.expected_uptime_seconds = 86400; // 100% uptime
        metrics.last_update_block = 11000;
        
        let result = classify_from_behavior(&metrics);
        assert_eq!(result.node_type, NodeType::Oracle);
    }

    #[test]
    fn test_manager_recording() {
        let mut manager = NodeClassificationManager::new(1000);
        
        manager.record_block_stored();
        manager.record_block_validated(100);
        manager.record_solution_submitted(true);
        manager.record_block_propagated();
        
        assert_eq!(manager.local_metrics.blocks_stored, 1);
        assert_eq!(manager.local_metrics.blocks_validated, 1);
        assert_eq!(manager.local_metrics.solutions_accepted, 1);
        assert_eq!(manager.local_metrics.blocks_propagated, 1);
    }

    #[test]
    fn test_stake_requirements() {
        // Light should require no stake
        assert_eq!(NodeType::Light.min_stake(), 0);
        
        // Validator should require most stake
        assert!(NodeType::Validator.min_stake() > NodeType::Full.min_stake());
        assert!(NodeType::Validator.min_stake() > NodeType::Bounty.min_stake());
    }
}

