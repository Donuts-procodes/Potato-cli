use serde::{Deserialize, Serialize};
use tracing::info;

use crate::agents::traits::{IssueSeverity, SubagentResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVerdict {
    pub approved: bool,
    pub quality_score: f32,
    pub security_score: f32,
    pub blocking_issues: Vec<String>,
}

/// Evaluates implementations using multi-agent critic consensus.
pub struct ConsensusEngine {
    confidence_threshold: f32,
}

impl ConsensusEngine {
    pub fn new(confidence_threshold: f32) -> Self {
        Self {
            confidence_threshold: confidence_threshold.clamp(0.1, 1.0),
        }
    }

    /// Evaluates the outputs of Reviewer and SecurityAuditor to form an objective verdict.
    pub fn evaluate(
        &self,
        review_result: &SubagentResult,
        security_result: &SubagentResult,
    ) -> ConsensusVerdict {
        let mut blocking_issues = Vec::new();

        // 1. Calculate Review Quality Score
        let mut quality_penalties = 0.0f32;
        for issue in &review_result.issues {
            match issue.severity {
                IssueSeverity::Critical => {
                    quality_penalties += 0.40;
                    blocking_issues.push(format!("[CODE QUALITY] {}", issue.description));
                }
                IssueSeverity::Warning => quality_penalties += 0.10,
                IssueSeverity::Info => quality_penalties += 0.02,
            }
        }
        let quality_score = (1.0 - quality_penalties).max(0.0);

        // 2. Calculate Security Score
        let mut security_penalties = 0.0f32;
        for issue in &security_result.issues {
            match issue.severity {
                IssueSeverity::Critical => {
                    security_penalties += 0.50;
                    blocking_issues.push(format!("[SECURITY] {}", issue.description));
                }
                IssueSeverity::Warning => security_penalties += 0.15,
                IssueSeverity::Info => security_penalties += 0.05,
            }
        }
        let security_score = (1.0 - security_penalties).max(0.0);

        // 3. Aggregate Verdict
        let overall_score = (quality_score * 0.5) + (security_score * 0.5);
        let approved = overall_score >= self.confidence_threshold && blocking_issues.is_empty();

        info!(
            quality_score,
            security_score,
            overall_score,
            approved,
            blocking_count = blocking_issues.len(),
            "Consensus evaluation completed"
        );

        ConsensusVerdict {
            approved,
            quality_score,
            security_score,
            blocking_issues,
        }
    }
}
