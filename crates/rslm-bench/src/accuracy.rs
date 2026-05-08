/// Shared accuracy evaluation types and scorer logic.
///
/// Used by both the deterministic Criterion bench and the live eval scripts.
use serde::{Deserialize, Serialize};

/// A single golden test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyCase {
    /// Unique identifier.
    pub id: String,
    /// Query sent to the RLM.
    pub query: String,
    /// One or more substrings/patterns that must appear in the answer (all must match).
    pub expected_patterns: Vec<String>,
    /// Optional: scripted ideal responses for the deterministic harness bench.
    /// If empty, the case is live-only.
    #[serde(default)]
    pub harness_responses: Vec<String>,
}

/// Result of evaluating one case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyResult {
    pub id: String,
    pub query: String,
    pub answer: String,
    pub passed: bool,
    /// Which patterns failed to match (empty if passed).
    pub failed_patterns: Vec<String>,
    pub latency_ms: u64,
}

impl AccuracyCase {
    /// Score an answer against this case's expected patterns (case-insensitive substring match).
    pub fn score(&self, answer: &str) -> AccuracyResult {
        let lower = answer.to_lowercase();
        let failed: Vec<String> = self
            .expected_patterns
            .iter()
            .filter(|p| !lower.contains(&p.to_lowercase()))
            .cloned()
            .collect();
        AccuracyResult {
            id: self.id.clone(),
            query: self.query.clone(),
            answer: answer.to_string(),
            passed: failed.is_empty(),
            failed_patterns: failed,
            latency_ms: 0,
        }
    }
}

/// Summary over a set of results.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccuracyReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    pub results: Vec<AccuracyResult>,
}

impl AccuracyReport {
    pub fn from_results(results: Vec<AccuracyResult>) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let pass_rate = if total == 0 {
            0.0
        } else {
            passed as f64 / total as f64
        };
        Self {
            total,
            passed,
            failed,
            pass_rate,
            results,
        }
    }
}
