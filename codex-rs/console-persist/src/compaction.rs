use serde::Deserialize;
use serde::Serialize;

/// Policy for when and how to compact context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPolicy {
    /// Maximum tokens before triggering compaction.
    pub max_context_tokens: u64,
    /// What fraction of context to keep as summary (0.0-1.0).
    pub summary_ratio: f64,
    /// Minimum messages to keep verbatim (recent messages).
    pub keep_recent: usize,
    /// Whether to include tool results in summaries.
    pub summarize_tool_results: bool,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self {
            max_context_tokens: 150_000,
            summary_ratio: 0.3,
            keep_recent: 10,
            summarize_tool_results: false,
        }
    }
}

/// A rolling summary segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingSummary {
    pub summary_text: String,
    pub messages_summarized: usize,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub created_at: String,
}

/// Result of a compaction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    pub summaries: Vec<RollingSummary>,
    pub messages_kept: usize,
    pub messages_compacted: usize,
    pub tokens_saved: u64,
}

/// Determines if compaction is needed and what messages to compact.
pub fn should_compact(total_tokens: u64, policy: &CompactionPolicy) -> bool {
    total_tokens > policy.max_context_tokens
}

/// Calculate how many messages to keep verbatim (from the end).
pub fn messages_to_keep(total_messages: usize, policy: &CompactionPolicy) -> usize {
    policy.keep_recent.min(total_messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let policy = CompactionPolicy::default();
        assert_eq!(policy.max_context_tokens, 150_000);
        assert!((policy.summary_ratio - 0.3).abs() < f64::EPSILON);
        assert_eq!(policy.keep_recent, 10);
        assert!(!policy.summarize_tool_results);
    }

    #[test]
    fn test_should_compact_triggers() {
        let policy = CompactionPolicy::default();
        assert!(!should_compact(100_000, &policy));
        assert!(!should_compact(150_000, &policy));
        assert!(should_compact(150_001, &policy));
        assert!(should_compact(200_000, &policy));
    }

    #[test]
    fn test_should_compact_custom_threshold() {
        let policy = CompactionPolicy {
            max_context_tokens: 50_000,
            ..Default::default()
        };
        assert!(!should_compact(50_000, &policy));
        assert!(should_compact(50_001, &policy));
    }

    #[test]
    fn test_messages_to_keep_respects_minimum() {
        let policy = CompactionPolicy::default(); // keep_recent = 10
        assert_eq!(messages_to_keep(100, &policy), 10);
        assert_eq!(messages_to_keep(10, &policy), 10);
        assert_eq!(messages_to_keep(5, &policy), 5);
        assert_eq!(messages_to_keep(0, &policy), 0);
    }

    #[test]
    fn test_messages_to_keep_custom_policy() {
        let policy = CompactionPolicy {
            keep_recent: 3,
            ..Default::default()
        };
        assert_eq!(messages_to_keep(100, &policy), 3);
        assert_eq!(messages_to_keep(2, &policy), 2);
    }

    #[test]
    fn test_policy_serialization() {
        let policy = CompactionPolicy::default();
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: CompactionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_context_tokens, policy.max_context_tokens);
        assert_eq!(deserialized.keep_recent, policy.keep_recent);
    }

    #[test]
    fn test_compaction_result_serialization() {
        let result = CompactionResult {
            summaries: vec![RollingSummary {
                summary_text: "User asked about Rust. Assistant explained ownership.".into(),
                messages_summarized: 8,
                tokens_before: 5000,
                tokens_after: 500,
                created_at: "2025-01-01T00:00:00Z".into(),
            }],
            messages_kept: 10,
            messages_compacted: 8,
            tokens_saved: 4500,
        };
        let json = serde_json::to_string_pretty(&result).unwrap();
        let deserialized: CompactionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.messages_kept, 10);
        assert_eq!(deserialized.messages_compacted, 8);
        assert_eq!(deserialized.tokens_saved, 4500);
        assert_eq!(deserialized.summaries.len(), 1);
        assert_eq!(deserialized.summaries[0].messages_summarized, 8);
    }
}
