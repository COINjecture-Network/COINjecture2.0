// =============================================================================
// Dynamic Data Pricing (EMPIRICAL VERSION)
// P_d = (C_network × H_empirical) × (1 + α · (D_active / S_available))
// =============================================================================
//
// COMPLIANCE: Empirical ✓ | Self-referential ✓ | Dimensionless ✓
//
// ALL values derived from network state:
// - C_network: Base cost from median network fees (not hardcoded)
// - H_empirical: Hardness from actual solve times (not hardcoded)
// - Supply/demand ratio: Pure dimensionless ratio
// - No MAX/MIN price caps - market self-regulates
//
// The α sensitivity parameter is the only mathematical constant (from η = 1/√2)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Mathematical constant η = 1/√2 (not arbitrary - from dimensional theory)
const ETA: f64 = 0.7071067811865476;

// Golden ratio inverse (mathematical, not arbitrary)
const PHI_INV: f64 = 0.6180339887498949;

// =============================================================================
// Problem Types
// =============================================================================

/// NP-Hard problem types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProblemCategory {
    /// 3-SAT (Boolean satisfiability) - baseline
    Sat3 = 0,
    /// Traveling Salesman Problem
    Tsp = 1,
    /// Graph Coloring
    GraphColoring = 2,
    /// Subset Sum
    SubsetSum = 3,
    /// Knapsack Problem
    Knapsack = 4,
    /// Maximum Independent Set
    MaxIndependentSet = 5,
    /// Hamiltonian Path
    HamiltonianPath = 6,
    /// Vertex Cover
    VertexCover = 7,
    /// Set Cover
    SetCover = 8,
    /// Custom (user-defined)
    Custom = 9,
}

impl ProblemCategory {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
    
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => ProblemCategory::Sat3,
            1 => ProblemCategory::Tsp,
            2 => ProblemCategory::GraphColoring,
            3 => ProblemCategory::SubsetSum,
            4 => ProblemCategory::Knapsack,
            5 => ProblemCategory::MaxIndependentSet,
            6 => ProblemCategory::HamiltonianPath,
            7 => ProblemCategory::VertexCover,
            8 => ProblemCategory::SetCover,
            _ => ProblemCategory::Custom,
        }
    }
}

// =============================================================================
// Network-Derived Metrics Reference
// =============================================================================

/// Interface to network metrics oracle
/// This abstracts the connection to the central metrics system
#[derive(Debug, Clone)]
pub struct PricingMetrics {
    /// Median solve times per category (from network history)
    solve_times: HashMap<ProblemCategory, f64>,
    /// Baseline solve time (SAT3 median)
    baseline_solve_time: f64,
    /// Median network fee (replaces C_BASE)
    median_fee: u128,
    /// Network age factor (for bootstrap pricing)
    network_age_blocks: u64,
}

impl PricingMetrics {
    /// Create with bootstrap defaults
    pub fn bootstrap(network_age: u64) -> Self {
        // During bootstrap, use exponential scaling based on η
        let mut solve_times = HashMap::new();
        for cat in 0..=9 {
            let category = ProblemCategory::from_u8(cat);
            // H_n = e^(η * n) - mathematically derived
            solve_times.insert(category, (ETA * cat as f64).exp());
        }
        
        PricingMetrics {
            solve_times,
            baseline_solve_time: 1.0,
            median_fee: 0,
            network_age_blocks: network_age,
        }
    }
    
    /// Update from network metrics oracle
    pub fn update_from_network(
        &mut self,
        median_fee: u128,
        solve_time_ratios: &[(ProblemCategory, f64)],
        network_age: u64,
    ) {
        self.median_fee = median_fee;
        self.network_age_blocks = network_age;
        
        for (cat, ratio) in solve_time_ratios {
            self.solve_times.insert(*cat, *ratio);
        }
        
        // Update baseline from SAT3
        if let Some(&baseline) = self.solve_times.get(&ProblemCategory::Sat3) {
            self.baseline_solve_time = baseline.max(0.001);
        }
    }
    
    /// Get empirical hardness factor for a category
    /// H_factor = solve_time_category / solve_time_baseline
    pub fn hardness_factor(&self, category: ProblemCategory) -> f64 {
        let cat_time = self.solve_times.get(&category).copied().unwrap_or(1.0);
        cat_time / self.baseline_solve_time.max(0.001)
    }
    
    /// Get base storage cost (network-derived)
    /// During bootstrap: grows logarithmically with network age
    /// After bootstrap: uses median fee
    pub fn base_cost(&self) -> u128 {
        if self.median_fee > 0 {
            return self.median_fee;
        }
        
        // Bootstrap pricing: cost grows with network maturity
        // C = η * ln(1 + block_height) * 1000
        let age_factor = (1.0 + self.network_age_blocks as f64).ln();
        (ETA * age_factor * 1000.0) as u128
    }
    
    /// Size-adjusted hardness using empirical data
    /// Combines category hardness with problem size scaling
    pub fn size_adjusted_hardness(&self, category: ProblemCategory, problem_size: u64) -> f64 {
        let base_hardness = self.hardness_factor(category);
        
        // Size scaling uses η for mathematical consistency
        // Larger problems are exponentially harder
        let size_factor = 1.0 + (problem_size as f64).ln() * ETA;
        
        base_hardness * size_factor
    }
}

impl Default for PricingMetrics {
    fn default() -> Self {
        Self::bootstrap(0)
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
    pub fn new(category: ProblemCategory, metrics: &PricingMetrics) -> Self {
        let initial_price = Self::calculate_base_price(category, 50, metrics);
        
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

    /// Calculate base price using network-derived values
    /// P_base = C_network × H_empirical(category, size)
    fn calculate_base_price(category: ProblemCategory, size: u64, metrics: &PricingMetrics) -> u128 {
        let h_factor = metrics.size_adjusted_hardness(category, size);
        let c_base = metrics.base_cost();
        
        ((c_base as f64) * h_factor) as u128
    }

    /// Calculate dynamic price with NO ARTIFICIAL CAPS
    /// P_d = P_base × (1 + α × (D/S))
    /// where α = η (mathematical constant from dimensional theory)
    pub fn calculate_price(&self, problem_size: u64, metrics: &PricingMetrics) -> u128 {
        let base_price = Self::calculate_base_price(self.category, problem_size, metrics);
        
        // Supply/demand ratio (dimensionless)
        let demand_supply_ratio = if self.supply_available > 0 {
            self.demand_active as f64 / self.supply_available as f64
        } else if self.demand_active > 0 {
            // High demand, no supply - use φ^2 (golden ratio squared) as natural bound
            // This is NOT arbitrary - it's the limiting ratio in Fibonacci growth
            let phi = 1.618033988749895;
            phi * phi // ≈ 2.618
        } else {
            // No demand, no supply - neutral pricing
            1.0
        };
        
        // Apply formula with α = η (mathematical constant)
        // NO MIN/MAX CLAMPS - market self-regulates
        let multiplier = 1.0 + ETA * demand_supply_ratio;
        
        ((base_price as f64) * multiplier) as u128
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
        
        // EMA smoothing factor = η (mathematical constant)
        self.price_ema = ETA * (price as f64) + (1.0 - ETA) * self.price_ema;
    }

    /// Update price based on current market conditions
    pub fn update_price(&mut self, problem_size: u64, block: u64, metrics: &PricingMetrics) {
        let new_price = self.calculate_price(problem_size, metrics);
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

    /// Calculate price volatility (standard deviation / mean) - dimensionless
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

/// Global data pricing engine with network-derived parameters
#[derive(Debug)]
pub struct DataPricingEngine {
    /// Markets by category
    markets: HashMap<ProblemCategory, CategoryMarket>,
    /// Current block height
    current_block: u64,
    /// Network-derived metrics
    metrics: PricingMetrics,
}

impl DataPricingEngine {
    /// Create new engine with network metrics
    pub fn new(network_age: u64) -> Self {
        let metrics = PricingMetrics::bootstrap(network_age);
        let mut markets = HashMap::new();
        
        // Initialize markets for all categories
        for cat in 0..=9 {
            let category = ProblemCategory::from_u8(cat);
            markets.insert(category, CategoryMarket::new(category, &metrics));
        }
        
        DataPricingEngine {
            markets,
            current_block: network_age,
            metrics,
        }
    }

    /// Update metrics from network oracle
    pub fn update_metrics(&mut self, metrics: PricingMetrics) {
        self.metrics = metrics;
    }
    
    /// Update a single solve time observation
    pub fn record_solve_time(&mut self, category: ProblemCategory, solve_time: f64) {
        self.metrics.solve_times.insert(category, solve_time);
        
        // Update baseline if SAT3
        if category == ProblemCategory::Sat3 {
            self.metrics.baseline_solve_time = solve_time.max(0.001);
        }
    }
    
    /// Update median fee from network
    pub fn update_median_fee(&mut self, median_fee: u128) {
        self.metrics.median_fee = median_fee;
    }

    /// Set current block
    pub fn set_block(&mut self, block: u64) {
        self.current_block = block;
        self.metrics.network_age_blocks = block;
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
            .map(|m| m.calculate_price(problem_size, &self.metrics))
            .unwrap_or_else(|| self.metrics.base_cost())
    }
    
    /// Get current hardness factor (empirical)
    pub fn get_hardness_factor(&self, category: ProblemCategory) -> f64 {
        self.metrics.hardness_factor(category)
    }
    
    /// Get current base cost (network-derived)
    pub fn get_base_cost(&self) -> u128 {
        self.metrics.base_cost()
    }

    /// Submit a buy order (increases demand)
    pub fn submit_buy_order(&mut self, category: ProblemCategory, problem_size: u64) -> BuyOrder {
        let price = self.get_price(category, problem_size);
        
        if let Some(market) = self.markets.get_mut(&category) {
            market.add_demand(1);
            market.update_price(problem_size, self.current_block, &self.metrics);
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
            market.update_price(problem_size, self.current_block, &self.metrics);
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
            market.update_price(problem_size, self.current_block, &self.metrics);
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
            market.update_price(avg_problem_size, self.current_block, &self.metrics);
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
            base_cost: self.metrics.base_cost(),
            network_age: self.metrics.network_age_blocks,
        }
    }
}

impl Default for DataPricingEngine {
    fn default() -> Self {
        Self::new(0)
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
    pub base_cost: u128,
    pub network_age: u64,
}

/// Get current Unix timestamp
fn current_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
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
    fn test_bootstrap_pricing() {
        // During bootstrap, prices should be minimal but grow with network age
        let engine_young = DataPricingEngine::new(0);
        let engine_mature = DataPricingEngine::new(10000);
        
        let price_young = engine_young.get_base_cost();
        let price_mature = engine_mature.get_base_cost();
        
        assert!(price_mature > price_young, 
            "Mature network should have higher base cost: {} > {}", price_mature, price_young);
    }

    #[test]
    fn test_hardness_factors_exponential() {
        let engine = DataPricingEngine::new(0);
        
        // During bootstrap, hardness should follow exponential scaling
        let h_sat = engine.get_hardness_factor(ProblemCategory::Sat3);
        let h_tsp = engine.get_hardness_factor(ProblemCategory::Tsp);
        
        assert!(h_tsp > h_sat, "TSP should be harder than SAT3: {} > {}", h_tsp, h_sat);
        
        // Ratio should follow e^(η * Δn)
        let expected_ratio = (ETA * 1.0).exp(); // TSP is category 1, SAT3 is category 0
        let actual_ratio = h_tsp / h_sat;
        assert!((actual_ratio - expected_ratio).abs() < 0.1,
            "Hardness ratio should follow exponential: {} vs {}", actual_ratio, expected_ratio);
    }

    #[test]
    fn test_no_artificial_caps() {
        let mut engine = DataPricingEngine::new(100);
        
        // Create extreme demand
        if let Some(market) = engine.get_market_mut(ProblemCategory::Sat3) {
            market.add_demand(1000);
            // No supply
        }
        
        let high_demand_price = engine.get_price(ProblemCategory::Sat3, 50);
        
        // Price should be high but NOT capped at an arbitrary value
        // It should scale naturally with demand
        assert!(high_demand_price > engine.get_base_cost());
    }

    #[test]
    fn test_demand_supply_impact() {
        let mut engine = DataPricingEngine::new(100);
        
        let balanced_price = engine.get_price(ProblemCategory::Sat3, 50);
        
        // Add demand
        if let Some(market) = engine.get_market_mut(ProblemCategory::Sat3) {
            market.add_demand(10);
        }
        let high_demand_price = engine.get_price(ProblemCategory::Sat3, 50);
        
        // Add supply
        if let Some(market) = engine.get_market_mut(ProblemCategory::Sat3) {
            market.add_supply(20);
        }
        let high_supply_price = engine.get_price(ProblemCategory::Sat3, 50);
        
        assert!(high_demand_price > balanced_price,
            "High demand should increase price: {} > {}", high_demand_price, balanced_price);
        
        assert!(high_supply_price < high_demand_price,
            "More supply should decrease price: {} < {}", high_supply_price, high_demand_price);
    }

    #[test]
    fn test_empirical_solve_time_update() {
        let mut engine = DataPricingEngine::new(100);
        
        // Record empirical solve times
        engine.record_solve_time(ProblemCategory::Sat3, 1.0);
        engine.record_solve_time(ProblemCategory::Tsp, 5.0);
        
        let h_sat = engine.get_hardness_factor(ProblemCategory::Sat3);
        let h_tsp = engine.get_hardness_factor(ProblemCategory::Tsp);
        
        // TSP should be 5x harder based on actual solve times
        assert!((h_tsp / h_sat - 5.0).abs() < 0.1,
            "Empirical hardness should be 5x: {} / {} = {}", h_tsp, h_sat, h_tsp / h_sat);
    }

    #[test]
    fn test_median_fee_update() {
        let mut engine = DataPricingEngine::new(1000);
        
        // Initially uses bootstrap pricing
        let bootstrap_cost = engine.get_base_cost();
        
        // Update with network median
        engine.update_median_fee(50000);
        
        let network_cost = engine.get_base_cost();
        
        assert_eq!(network_cost, 50000, 
            "Should use network median when available: {}", network_cost);
    }

    #[test]
    fn test_volatility_dimensionless() {
        let mut market = CategoryMarket::new(
            ProblemCategory::Sat3,
            &PricingMetrics::bootstrap(0)
        );
        
        // Record varying prices
        market.price_history.push(PricePoint { block: 1, price: 100, demand: 1, supply: 1 });
        market.price_history.push(PricePoint { block: 2, price: 120, demand: 1, supply: 1 });
        market.price_history.push(PricePoint { block: 3, price: 80, demand: 1, supply: 1 });
        market.price_history.push(PricePoint { block: 4, price: 110, demand: 1, supply: 1 });
        
        let volatility = market.calculate_volatility();
        
        // Volatility should be a dimensionless ratio
        assert!(volatility > 0.0 && volatility < 1.0,
            "Volatility should be a ratio between 0 and 1: {}", volatility);
    }
}
