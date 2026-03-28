// =============================================================================
// Load Test Results — Structured Output & Template
// =============================================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::Utc;

/// Top-level test result container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    /// Test name / command
    pub test_name: String,
    /// Whether the test passed its success criteria
    pub passed: bool,
    /// ISO-8601 start timestamp
    pub started_at: String,
    /// ISO-8601 end timestamp
    pub ended_at: String,
    /// Elapsed seconds
    pub elapsed_secs: f64,
    /// High-level summary
    pub summary: String,
    /// Numeric metrics (latency, throughput, error rate, etc.)
    pub metrics: HashMap<String, f64>,
    /// Human-readable metric labels (unit annotations)
    pub metric_labels: HashMap<String, String>,
    /// Per-phase results (for multi-phase tests like stability)
    pub phases: Vec<PhaseResult>,
    /// Errors encountered (message, count)
    pub errors: Vec<ErrorEntry>,
    /// Additional free-form notes
    pub notes: Vec<String>,
}

impl TestResults {
    pub fn new(test_name: impl Into<String>) -> Self {
        TestResults {
            test_name: test_name.into(),
            passed: false,
            started_at: Utc::now().to_rfc3339(),
            ended_at: String::new(),
            elapsed_secs: 0.0,
            summary: String::new(),
            metrics: HashMap::new(),
            metric_labels: HashMap::new(),
            phases: Vec::new(),
            errors: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn finish(&mut self, passed: bool, summary: impl Into<String>, elapsed_secs: f64) {
        self.passed = passed;
        self.summary = summary.into();
        self.ended_at = Utc::now().to_rfc3339();
        self.elapsed_secs = elapsed_secs;
    }

    pub fn metric(&mut self, key: impl Into<String>, value: f64, label: impl Into<String>) {
        let k = key.into();
        self.metrics.insert(k.clone(), value);
        self.metric_labels.insert(k, label.into());
    }

    pub fn error(&mut self, message: impl Into<String>, count: u64) {
        self.errors.push(ErrorEntry { message: message.into(), count });
    }

    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    pub fn print_summary(&self) {
        let status = if self.passed { "PASS" } else { "FAIL" };
        println!("\n═══════════════════════════════════════════════════════════");
        println!(" Load Test: {}  [{status}]", self.test_name);
        println!("═══════════════════════════════════════════════════════════");
        println!(" Duration : {:.1}s", self.elapsed_secs);
        println!(" Summary  : {}", self.summary);

        if !self.metrics.is_empty() {
            println!("\n Metrics:");
            let mut keys: Vec<_> = self.metrics.keys().collect();
            keys.sort();
            for k in keys {
                let v = self.metrics[k];
                let label = self.metric_labels.get(k).map(|s| s.as_str()).unwrap_or("");
                println!("   {:<35} {:>10.2}  {}", k, v, label);
            }
        }

        if !self.phases.is_empty() {
            println!("\n Phases:");
            for phase in &self.phases {
                let pstatus = if phase.passed { "✓" } else { "✗" };
                println!("   {pstatus} {} ({:.1}s): {}", phase.name, phase.elapsed_secs, phase.summary);
            }
        }

        if !self.errors.is_empty() {
            println!("\n Errors:");
            for e in &self.errors {
                println!("   [{}×] {}", e.count, e.message);
            }
        }

        if !self.notes.is_empty() {
            println!("\n Notes:");
            for n in &self.notes {
                println!("   • {}", n);
            }
        }

        println!("═══════════════════════════════════════════════════════════\n");
    }
}

/// Result for a single test phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub name: String,
    pub passed: bool,
    pub elapsed_secs: f64,
    pub summary: String,
    pub metrics: HashMap<String, f64>,
}

/// An error type and how many times it occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub message: String,
    pub count: u64,
}

/// Latency histogram (P50/P95/P99/Max).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub samples: Vec<f64>,
}

impl LatencyStats {
    pub fn record(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn p50(&self) -> f64 { self.percentile(0.50) }
    pub fn p95(&self) -> f64 { self.percentile(0.95) }
    pub fn p99(&self) -> f64 { self.percentile(0.99) }
    pub fn max(&self) -> f64 { self.samples.iter().cloned().fold(0.0_f64, f64::max) }
    pub fn min(&self) -> f64 { self.samples.iter().cloned().fold(f64::MAX, f64::min) }
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
        sorted[idx]
    }

    pub fn apply_to_results(&self, results: &mut TestResults, prefix: &str) {
        results.metric(format!("{prefix}.p50_ms"), self.p50(), "ms");
        results.metric(format!("{prefix}.p95_ms"), self.p95(), "ms");
        results.metric(format!("{prefix}.p99_ms"), self.p99(), "ms");
        results.metric(format!("{prefix}.max_ms"), self.max(), "ms");
        results.metric(format!("{prefix}.mean_ms"), self.mean(), "ms");
        results.metric(format!("{prefix}.count"), self.samples.len() as f64, "samples");
    }
}

/// Simple throughput counter.
#[derive(Debug, Clone, Default)]
pub struct ThroughputCounter {
    pub total: u64,
    pub errors: u64,
    pub start: Option<std::time::Instant>,
}

impl ThroughputCounter {
    pub fn start(&mut self) {
        self.start = Some(std::time::Instant::now());
    }

    pub fn success(&mut self) { self.total += 1; }
    pub fn fail(&mut self)    { self.total += 1; self.errors += 1; }

    pub fn tps(&self) -> f64 {
        let elapsed = self.start.map(|s| s.elapsed().as_secs_f64()).unwrap_or(1.0);
        if elapsed > 0.0 { self.total as f64 / elapsed } else { 0.0 }
    }

    pub fn error_rate(&self) -> f64 {
        if self.total == 0 { return 0.0; }
        self.errors as f64 / self.total as f64
    }

    pub fn apply_to_results(&self, results: &mut TestResults, prefix: &str) {
        results.metric(format!("{prefix}.total"), self.total as f64, "ops");
        results.metric(format!("{prefix}.errors"), self.errors as f64, "ops");
        results.metric(format!("{prefix}.tps"), self.tps(), "ops/s");
        results.metric(format!("{prefix}.error_rate"), self.error_rate() * 100.0, "%");
    }
}
