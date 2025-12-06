// =============================================================================
// Dynamic Data Pricing
// P_d = (C_base × H_factor) × (1 + α · (D_active / S_available))
// =============================================================================
//
// Price adjusts automatically based on:
// - C_base: Base storage cost (computational floor)
// - H_factor: NP-Hardness multiplier (problem complexity)
// - D_active: Active buy orders (demand)
// - S_available: Verified solution supply
// - α: Supply/demand sensitivity parameter

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Constants
// =============================================================================

/// Base storage cost in tokens (minimum price floor)
pub const C_BASE: u128 = 100_000; // 0.1 tokens (6 decimals)

/// Supply/demand sensitivity parameter α
pub const ALPHA: f64 = 0.5;

/// Maximum price multiplier (prevents runaway prices)
pub const MAX_PRICE_MULTIPLIER: f64 = 100.0;

/// Minimum price multiplier (floor)
pub const MIN_PRICE_MULTIPLIER: f64 = 0.1;

/// Price update interval (blocks)
pub const PRICE_UPDATE_INTERVAL: u64 = 10;

/// EMA smoothing factor for price updates
pub const EMA_ALPHA: f64 = 0.1;

// =============================================================================
// Problem Types & Hardness
// =============================================================================

/// NP-Hard problem types with hardness factors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProblemCategory {
    /// 3-SAT (Boolean satisfiability)
    Sat3,
    /// Traveling Salesman Problem
    Tsp,
    /// Graph Coloring
    GraphColoring,
    /// Subset Sum
    SubsetSum,
    /// Knapsack Problem
    Knapsack,
    /// Maximum Independent Set
    MaxIndependentSet,
    /// Hamiltonian Path
    HamiltonianPath,
    /// Vertex Cover
    VertexCover,
    /// Set Cover
    SetCover,
    /// Custom (user-defined)
    Custom,
}

impl ProblemCategory {
    /// Get base hardness factor H for this problem type
    /// Higher = more computationally expensive
    pub fn hardness_factor(&self) -> f64 {
        match self {
            ProblemCategory::Sat3 => 1.0,             // Baseline
            ProblemCategory::Tsp => 2.5,              // Very hard
            ProblemCategory::GraphColoring => 1.5,
            ProblemCategory::SubsetSum => 1.2,
            ProblemCategory::Knapsack => 1.3,
            ProblemCategory::MaxIndependentSet => 1.8,
            ProblemCategory::HamiltonianPath => 2.2,
            ProblemCategory::VertexCover => 1.6,
            ProblemCategory::SetCover => 1.7,
            ProblemCategory::Custom => 1.0,
        }
    }

    /// Get size-based hardness scaling
    /// Returns H_factor = H_base × size_multiplier
    pub fn size_adjusted_hardness(&self, problem_size: u64) -> f64 {
        let base = self.hardness_factor();
        
        // Exponential scaling with problem size
        // For NP-hard problems, difficulty grows exponentially
        let size_factor = match self {
            // Polynomial-like scaling for smaller instances
            ProblemCategory::SubsetSum | ProblemCategory::Knapsack => {
                1.0 + (problem_size as f64).log2() * 0.2
            }
            // Exponential scaling for graph problems
            ProblemCategory::Tsp | ProblemCategory::HamiltonianPath => {
                1.0 + (problem_size as f64).powf(0.5) * 0.1
            }
            // Standard scaling
            _ => {
                1.0 + (problem_size as f64).log2() * 0.3
            }
        };
        
        base * size_factor
    }
}

// =============================================================================
// Market State
// =============================================================================

/// Current market state for a problem category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMarket {
    /// Problem category
    pub category: ProblemCategory,
    /// Active buy orders (demand)
    pub demand_active: u64,
    /// Available verified solutions (supply)
    pub supply_available: u64,
    /// Current price
    pub current_price: u128,
    /// Price EMA (smoothed)
    pub price_ema: f64,
    /// Total volume traded
    pub total_volume: u128,
    /// Number of trades
    pub trade_count: u64,
    /// Last update block
    pub last_update_block: u64,
    /// Price history (last 100 prices)
    pub price_history: Vec<PricePoint>,
}

/// Historical price point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub block: u64,
    pub price: u128,
    pub demand: u64,
    pub supply: u64,
}

impl CategoryMarket {
    /// Create new market for category
    pub fn new(category: ProblemCategory) -> Self {
        let initial_price = Self::calculate_base_price(category, 50); // Average size
        
        CategoryMarket {
            category,
            demand_active: 0,
            supply_available: 0,
            current_price: initial_price,
            price_ema: initial_price as f64,
            total_volume: 0,
            trade_count: 0,
            last_update_block: 0,
            price_history: Vec::new(),
        }
    }

    /// Calculate base price: C_base × H_factor
    fn calculate_base_price(category: ProblemCategory, size: u64) -> u128 {
        let h_factor = category.size_adjusted_hardness(size);
        ((C_BASE as f64) * h_factor) as u128
    }

    /// Calculate dynamic price: P_d = (C_base × H_factor) × (1 + α × (D/S))
    pub fn calculate_price(&self, problem_size: u64) -> u128 {
        let base_price = Self::calculate_base_price(self.category, problem_size);
        
        // Supply/demand ratio
        let demand_supply_ratio = if self.supply_available > 0 {
            self.demand_active as f64 / self.supply_available as f64
        } else if self.demand_active > 0 {
            // No supply but demand exists - use max multiplier
            MAX_PRICE_MULTIPLIER
        } else {
            // No demand, no supply - neutral
            1.0
        };
        
        // Apply formula: P_d = base × (1 + α × (D/S))
        let multiplier = 1.0 + ALPHA * demand_supply_ratio;
        let clamped_multiplier = multiplier.clamp(MIN_PRICE_MULTIPLIER, MAX_PRICE_MULTIPLIER);
        
        ((base_price as f64) * clamped_multiplier) as u128
    }

    /// Add demand (new buy order)
    pub fn add_demand(&mut self, count: u64) {
        self.demand_active += count;
    }

    /// Remove demand (order filled or cancelled)
    pub fn remove_demand(&mut self, count: u64) {
        self.demand_active = self.demand_active.saturating_sub(count);
    }

    /// Add supply (new verified solution)
    pub fn add_supply(&mut self, count: u64) {
        self.supply_available += count;
    }

    /// Remove supply (solution purchased)
    pub fn remove_supply(&mut self, count: u64) {
        self.supply_available = self.supply_available.saturating_sub(count);
    }

    /// Record a trade
    pub fn record_trade(&mut self, price: u128, block: u64) {
        self.total_volume += price;
        self.trade_count += 1;
        self.last_update_block = block;
        
        // Update EMA
        self.price_ema = EMA_ALPHA * (price as f64) + (1.0 - EMA_ALPHA) * self.price_ema;
    }

    /// Update price based on current market conditions
    pub fn update_price(&mut self, problem_size: u64, block: u64) {
        let new_price = self.calculate_price(problem_size);
        self.current_price = new_price;
        
        // Record in history
        self.price_history.push(PricePoint {
            block,
            price: new_price,
            demand: self.demand_active,
            supply: self.supply_available,
        });
        
        // Trim history to last 100 entries
        if self.price_history.len() > 100 {
            self.price_history.remove(0);
        }
        
        self.last_update_block = block;
    }

    /// Get price statistics
    pub fn stats(&self) -> MarketStats {
        let avg_price = if !self.price_history.is_empty() {
            self.price_history.iter().map(|p| p.price).sum::<u128>() 
                / self.price_history.len() as u128
        } else {
            self.current_price
        };
        
        let volatility = self.calculate_volatility();
        
        MarketStats {
            category: self.category,
            current_price: self.current_price,
            price_ema: self.price_ema,
            avg_price_24h: avg_price,
            demand_active: self.demand_active,
            supply_available: self.supply_available,
            demand_supply_ratio: if self.supply_available > 0 {
                self.demand_active as f64 / self.supply_available as f64
            } else {
                f64::INFINITY
            },
            total_volume: self.total_volume,
            trade_count: self.trade_count,
            volatility,
        }
    }

    /// Calculate price volatility (standard deviation / mean)
    fn calculate_volatility(&self) -> f64 {
        if self.price_history.len() < 2 {
            return 0.0;
        }
        
        let prices: Vec<f64> = self.price_history.iter()
            .map(|p| p.price as f64)
            .collect();
        
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        
        let std_dev = variance.sqrt();
        if mean > 0.0 { std_dev / mean } else { 0.0 }
    }
}

/// Market statistics for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub category: ProblemCategory,
    pub current_price: u128,
    pub price_ema: f64,
    pub avg_price_24h: u128,
    pub demand_active: u64,
    pub supply_available: u64,
    pub demand_supply_ratio: f64,
    pub total_volume: u128,
    pub trade_count: u64,
    pub volatility: f64,
}

// =============================================================================
// Data Pricing Engine
// =============================================================================

/// Global data pricing engine
#[derive(Debug)]
pub struct DataPricingEngine {
    /// Markets by category
    markets: HashMap<ProblemCategory, CategoryMarket>,
    /// Current block height
    current_block: u64,
    /// Configuration
    config: PricingConfig,
}

/// Pricing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    /// Base cost
    pub c_base: u128,
    /// Supply/demand sensitivity
    pub alpha: f64,
    /// Maximum price multiplier
    pub max_multiplier: f64,
    /// Minimum price multiplier
    pub min_multiplier: f64,
    /// Price update interval (blocks)
    pub update_interval: u64,
}

impl Default for PricingConfig {
    fn default() -> Self {
        PricingConfig {
            c_base: C_BASE,
            alpha: ALPHA,
            max_multiplier: MAX_PRICE_MULTIPLIER,
            min_multiplier: MIN_PRICE_MULTIPLIER,
            update_interval: PRICE_UPDATE_INTERVAL,
        }
    }
}

impl DataPricingEngine {
    pub fn new() -> Self {
        Self::with_config(PricingConfig::default())
    }

    pub fn with_config(config: PricingConfig) -> Self {
        let mut markets = HashMap::new();
        
        // Initialize markets for all categories
        for category in &[
            ProblemCategory::Sat3,
            ProblemCategory::Tsp,
            ProblemCategory::GraphColoring,
            ProblemCategory::SubsetSum,
            ProblemCategory::Knapsack,
            ProblemCategory::MaxIndependentSet,
            ProblemCategory::HamiltonianPath,
            ProblemCategory::VertexCover,
            ProblemCategory::SetCover,
            ProblemCategory::Custom,
        ] {
            markets.insert(*category, CategoryMarket::new(*category));
        }
        
        DataPricingEngine {
            markets,
            current_block: 0,
            config,
        }
    }

    /// Set current block
    pub fn set_block(&mut self, block: u64) {
        self.current_block = block;
    }

    /// Get market for category
    pub fn get_market(&self, category: ProblemCategory) -> Option<&CategoryMarket> {
        self.markets.get(&category)
    }

    /// Get mutable market for category
    pub fn get_market_mut(&mut self, category: ProblemCategory) -> Option<&mut CategoryMarket> {
        self.markets.get_mut(&category)
    }

    /// Calculate price for a specific problem
    pub fn get_price(&self, category: ProblemCategory, problem_size: u64) -> u128 {
        self.markets
            .get(&category)
            .map(|m| m.calculate_price(problem_size))
            .unwrap_or(C_BASE)
    }

    /// Submit a buy order (increases demand)
    pub fn submit_buy_order(&mut self, category: ProblemCategory, problem_size: u64) -> BuyOrder {
        let price = self.get_price(category, problem_size);
        
        if let Some(market) = self.markets.get_mut(&category) {
            market.add_demand(1);
            market.update_price(problem_size, self.current_block);
        }
        
        BuyOrder {
            category,
            problem_size,
            price,
            block: self.current_block,
            timestamp: current_timestamp(),
        }
    }

    /// Submit a solution (increases supply)
    pub fn submit_solution(&mut self, category: ProblemCategory, problem_size: u64) {
        if let Some(market) = self.markets.get_mut(&category) {
            market.add_supply(1);
            market.update_price(problem_size, self.current_block);
        }
    }

    /// Execute a trade (matches buy order with solution)
    pub fn execute_trade(
        &mut self, 
        category: ProblemCategory, 
        problem_size: u64,
        agreed_price: u128,
    ) -> TradeResult {
        let market_price = self.get_price(category, problem_size);
        
        if let Some(market) = self.markets.get_mut(&category) {
            market.remove_demand(1);
            market.remove_supply(1);
            market.record_trade(agreed_price, self.current_block);
            market.update_price(problem_size, self.current_block);
        }
        
        TradeResult {
            category,
            market_price,
            agreed_price,
            block: self.current_block,
            slippage: if market_price > 0 {
                ((agreed_price as f64) - (market_price as f64)).abs() / (market_price as f64)
            } else {
                0.0
            },
        }
    }

    /// Update all markets (called periodically)
    pub fn update_all_markets(&mut self, avg_problem_size: u64) {
        for market in self.markets.values_mut() {
            market.update_price(avg_problem_size, self.current_block);
        }
    }

    /// Get all market statistics
    pub fn all_stats(&self) -> Vec<MarketStats> {
        self.markets.values().map(|m| m.stats()).collect()
    }

    /// Get global pricing statistics
    pub fn global_stats(&self) -> GlobalPricingStats {
        let total_demand: u64 = self.markets.values().map(|m| m.demand_active).sum();
        let total_supply: u64 = self.markets.values().map(|m| m.supply_available).sum();
        let total_volume: u128 = self.markets.values().map(|m| m.total_volume).sum();
        let total_trades: u64 = self.markets.values().map(|m| m.trade_count).sum();
        
        let avg_price: u128 = if !self.markets.is_empty() {
            self.markets.values().map(|m| m.current_price).sum::<u128>() 
                / self.markets.len() as u128
        } else {
            0
        };
        
        GlobalPricingStats {
            total_demand,
            total_supply,
            global_ratio: if total_supply > 0 {
                total_demand as f64 / total_supply as f64
            } else {
                0.0
            },
            avg_price,
            total_volume,
            total_trades,
            active_markets: self.markets.len(),
        }
    }
}

impl Default for DataPricingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Buy order result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyOrder {
    pub category: ProblemCategory,
    pub problem_size: u64,
    pub price: u128,
    pub block: u64,
    pub timestamp: i64,
}

/// Trade execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResult {
    pub category: ProblemCategory,
    pub market_price: u128,
    pub agreed_price: u128,
    pub block: u64,
    pub slippage: f64,
}

/// Global pricing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPricingStats {
    pub total_demand: u64,
    pub total_supply: u64,
    pub global_ratio: f64,
    pub avg_price: u128,
    pub total_volume: u128,
    pub total_trades: u64,
    pub active_markets: usize,
}

/// Get current Unix timestamp
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardness_factors() {
        assert_eq!(ProblemCategory::Sat3.hardness_factor(), 1.0);
        assert!(ProblemCategory::Tsp.hardness_factor() > ProblemCategory::Sat3.hardness_factor());
    }

    #[test]
    fn test_size_adjusted_hardness() {
        let small = ProblemCategory::Tsp.size_adjusted_hardness(10);
        let large = ProblemCategory::Tsp.size_adjusted_hardness(100);
        
        assert!(large > small, "Larger problems should be harder");
    }

    #[test]
    fn test_base_price_calculation() {
        let market = CategoryMarket::new(ProblemCategory::Sat3);
        let price = market.calculate_price(50);
        
        assert!(price >= C_BASE, "Price should be at least C_BASE");
    }

    #[test]
    fn test_demand_supply_impact() {
        let mut market = CategoryMarket::new(ProblemCategory::Sat3);
        
        let price_balanced = market.calculate_price(50);
        
        // Add demand
        market.add_demand(10);
        let price_high_demand = market.calculate_price(50);
        
        // Add supply
        market.add_supply(20);
        let price_high_supply = market.calculate_price(50);
        
        // High demand should increase price
        assert!(price_high_demand > price_balanced,
            "High demand should increase price: {} > {}", price_high_demand, price_balanced);
        
        // More supply should decrease price
        assert!(price_high_supply < price_high_demand,
            "More supply should decrease price: {} < {}", price_high_supply, price_high_demand);
    }

    #[test]
    fn test_trade_recording() {
        let mut market = CategoryMarket::new(ProblemCategory::Sat3);
        
        market.add_demand(1);
        market.add_supply(1);
        market.record_trade(1_000_000, 100);
        
        assert_eq!(market.trade_count, 1);
        assert_eq!(market.total_volume, 1_000_000);
    }

    #[test]
    fn test_engine_pricing() {
        let mut engine = DataPricingEngine::new();
        engine.set_block(100);
        
        // Get prices for different categories
        let sat_price = engine.get_price(ProblemCategory::Sat3, 50);
        let tsp_price = engine.get_price(ProblemCategory::Tsp, 50);
        
        // TSP should be more expensive (higher hardness)
        assert!(tsp_price > sat_price,
            "TSP should be more expensive: {} > {}", tsp_price, sat_price);
    }

    #[test]
    fn test_buy_order() {
        let mut engine = DataPricingEngine::new();
        engine.set_block(100);
        
        let order = engine.submit_buy_order(ProblemCategory::Sat3, 50);
        
        assert!(order.price > 0);
        assert_eq!(order.category, ProblemCategory::Sat3);
        
        // Demand should have increased
        let market = engine.get_market(ProblemCategory::Sat3).unwrap();
        assert_eq!(market.demand_active, 1);
    }

    #[test]
    fn test_trade_execution() {
        let mut engine = DataPricingEngine::new();
        engine.set_block(100);
        
        engine.submit_buy_order(ProblemCategory::Sat3, 50);
        engine.submit_solution(ProblemCategory::Sat3, 50);
        
        let result = engine.execute_trade(ProblemCategory::Sat3, 50, 150_000);
        
        assert!(result.market_price > 0);
        
        // Demand and supply should have decreased
        let market = engine.get_market(ProblemCategory::Sat3).unwrap();
        assert_eq!(market.demand_active, 0);
        assert_eq!(market.supply_available, 0);
    }

    #[test]
    fn test_global_stats() {
        let mut engine = DataPricingEngine::new();
        engine.set_block(100);
        
        engine.submit_buy_order(ProblemCategory::Sat3, 50);
        engine.submit_buy_order(ProblemCategory::Tsp, 30);
        engine.submit_solution(ProblemCategory::Sat3, 50);
        
        let stats = engine.global_stats();
        
        assert_eq!(stats.total_demand, 2);
        assert_eq!(stats.total_supply, 1);
        assert_eq!(stats.active_markets, 10); // All categories initialized
    }
}

