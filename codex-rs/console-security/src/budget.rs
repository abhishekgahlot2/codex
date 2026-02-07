use serde::{Deserialize, Serialize};

/// Performance budget limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBudget {
    /// Maximum tool call latency in ms (p99).
    pub tool_latency_p99_ms: u64,
    /// Maximum memory usage in MB.
    pub memory_ceiling_mb: u64,
    /// Maximum CPU time per turn in seconds.
    pub cpu_budget_secs: u64,
    /// Maximum response time in ms.
    pub response_time_p95_ms: u64,
}

impl Default for PerformanceBudget {
    fn default() -> Self {
        Self {
            tool_latency_p99_ms: 5000,
            memory_ceiling_mb: 512,
            cpu_budget_secs: 300,
            response_time_p95_ms: 30000,
        }
    }
}

/// A budget violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetViolation {
    pub metric: String,
    pub limit: u64,
    pub actual: u64,
    pub severity: ViolationSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationSeverity {
    Warning,
    Critical,
}

impl PerformanceBudget {
    /// Check a metric against the budget.
    pub fn check_tool_latency(&self, latency_ms: u64) -> Option<BudgetViolation> {
        if latency_ms > self.tool_latency_p99_ms {
            Some(BudgetViolation {
                metric: "tool_latency_p99".into(),
                limit: self.tool_latency_p99_ms,
                actual: latency_ms,
                severity: if latency_ms > self.tool_latency_p99_ms * 2 {
                    ViolationSeverity::Critical
                } else {
                    ViolationSeverity::Warning
                },
            })
        } else {
            None
        }
    }

    pub fn check_memory(&self, memory_mb: u64) -> Option<BudgetViolation> {
        if memory_mb > self.memory_ceiling_mb {
            Some(BudgetViolation {
                metric: "memory".into(),
                limit: self.memory_ceiling_mb,
                actual: memory_mb,
                severity: if memory_mb > self.memory_ceiling_mb * 2 {
                    ViolationSeverity::Critical
                } else {
                    ViolationSeverity::Warning
                },
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_budget_values() {
        let budget = PerformanceBudget::default();
        assert_eq!(budget.tool_latency_p99_ms, 5000);
        assert_eq!(budget.memory_ceiling_mb, 512);
        assert_eq!(budget.cpu_budget_secs, 300);
        assert_eq!(budget.response_time_p95_ms, 30000);
    }

    #[test]
    fn test_tool_latency_within_budget() {
        let budget = PerformanceBudget::default();
        assert!(budget.check_tool_latency(1000).is_none());
        assert!(budget.check_tool_latency(5000).is_none());
    }

    #[test]
    fn test_tool_latency_warning() {
        let budget = PerformanceBudget::default();
        let violation = budget.check_tool_latency(6000);
        assert!(violation.is_some());
        let v = violation.unwrap();
        assert_eq!(v.metric, "tool_latency_p99");
        assert_eq!(v.limit, 5000);
        assert_eq!(v.actual, 6000);
        assert_eq!(v.severity, ViolationSeverity::Warning);
    }

    #[test]
    fn test_tool_latency_critical() {
        let budget = PerformanceBudget::default();
        let violation = budget.check_tool_latency(11000);
        assert!(violation.is_some());
        let v = violation.unwrap();
        assert_eq!(v.severity, ViolationSeverity::Critical);
    }

    #[test]
    fn test_memory_within_budget() {
        let budget = PerformanceBudget::default();
        assert!(budget.check_memory(256).is_none());
        assert!(budget.check_memory(512).is_none());
    }

    #[test]
    fn test_memory_warning() {
        let budget = PerformanceBudget::default();
        let violation = budget.check_memory(700);
        assert!(violation.is_some());
        let v = violation.unwrap();
        assert_eq!(v.metric, "memory");
        assert_eq!(v.limit, 512);
        assert_eq!(v.actual, 700);
        assert_eq!(v.severity, ViolationSeverity::Warning);
    }

    #[test]
    fn test_memory_critical() {
        let budget = PerformanceBudget::default();
        let violation = budget.check_memory(1100);
        assert!(violation.is_some());
        let v = violation.unwrap();
        assert_eq!(v.severity, ViolationSeverity::Critical);
    }

    #[test]
    fn test_budget_serialization_roundtrip() {
        let budget = PerformanceBudget::default();
        let json = serde_json::to_string(&budget).unwrap();
        let deserialized: PerformanceBudget = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tool_latency_p99_ms, 5000);
        assert_eq!(deserialized.memory_ceiling_mb, 512);
        assert_eq!(deserialized.cpu_budget_secs, 300);
        assert_eq!(deserialized.response_time_p95_ms, 30000);
    }

    #[test]
    fn test_violation_serialization() {
        let violation = BudgetViolation {
            metric: "tool_latency_p99".into(),
            limit: 5000,
            actual: 8000,
            severity: ViolationSeverity::Warning,
        };
        let json = serde_json::to_string(&violation).unwrap();
        let deserialized: BudgetViolation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.metric, "tool_latency_p99");
        assert_eq!(deserialized.severity, ViolationSeverity::Warning);
    }

    #[test]
    fn test_severity_serialization() {
        let warning = serde_json::to_string(&ViolationSeverity::Warning).unwrap();
        assert_eq!(warning, "\"warning\"");
        let critical = serde_json::to_string(&ViolationSeverity::Critical).unwrap();
        assert_eq!(critical, "\"critical\"");
    }
}
