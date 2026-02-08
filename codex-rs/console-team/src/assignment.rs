use serde::Deserialize;
use serde::Serialize;

/// Strategy for assigning tasks to teammates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentStrategy {
    /// Lead manually assigns each task.
    Manual,
    /// Tasks are assigned round-robin among active teammates.
    RoundRobin,
    /// Tasks are assigned to the teammate with the fewest active tasks.
    LeastBusy,
}

impl Default for AssignmentStrategy {
    fn default() -> Self {
        Self::Manual
    }
}

/// Tracks assignment state for round-robin strategy.
#[derive(Debug, Clone)]
pub struct TaskAssigner {
    strategy: AssignmentStrategy,
    /// Index for round-robin assignment.
    round_robin_index: usize,
}

impl TaskAssigner {
    pub fn new(strategy: AssignmentStrategy) -> Self {
        Self {
            strategy,
            round_robin_index: 0,
        }
    }

    /// Pick the next assignee based on the current strategy.
    /// `agents` is the list of active agent IDs, `task_counts` maps agent_id to active task count.
    pub fn pick_assignee(
        &mut self,
        agents: &[String],
        task_counts: &std::collections::HashMap<String, usize>,
    ) -> Option<String> {
        if agents.is_empty() {
            return None;
        }

        match self.strategy {
            AssignmentStrategy::Manual => None, // Lead decides
            AssignmentStrategy::RoundRobin => {
                let idx = self.round_robin_index % agents.len();
                self.round_robin_index += 1;
                Some(agents[idx].clone())
            }
            AssignmentStrategy::LeastBusy => agents
                .iter()
                .min_by_key(|id| task_counts.get(id.as_str()).copied().unwrap_or(0))
                .cloned(),
        }
    }

    /// Current strategy.
    pub fn strategy(&self) -> AssignmentStrategy {
        self.strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_manual_returns_none() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::Manual);
        let agents = vec!["a".to_string(), "b".to_string()];
        let counts = HashMap::new();
        assert_eq!(assigner.pick_assignee(&agents, &counts), None);
    }

    #[test]
    fn test_round_robin_cycles() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::RoundRobin);
        let agents = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let counts = HashMap::new();

        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("a".to_string())
        );
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("b".to_string())
        );
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("c".to_string())
        );
    }

    #[test]
    fn test_round_robin_wraps() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::RoundRobin);
        let agents = vec!["a".to_string(), "b".to_string()];
        let counts = HashMap::new();

        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("a".to_string())
        );
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("b".to_string())
        );
        // Should wrap around
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("a".to_string())
        );
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("b".to_string())
        );
    }

    #[test]
    fn test_least_busy_picks_min() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::LeastBusy);
        let agents = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut counts = HashMap::new();
        counts.insert("a".to_string(), 3);
        counts.insert("b".to_string(), 1);
        counts.insert("c".to_string(), 5);

        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("b".to_string())
        );
    }

    #[test]
    fn test_least_busy_tie_break() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::LeastBusy);
        let agents = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut counts = HashMap::new();
        counts.insert("a".to_string(), 2);
        counts.insert("b".to_string(), 2);
        counts.insert("c".to_string(), 2);

        // On tie, min_by_key returns the first element
        assert_eq!(
            assigner.pick_assignee(&agents, &counts),
            Some("a".to_string())
        );
    }

    #[test]
    fn test_empty_agents_returns_none() {
        let mut assigner = TaskAssigner::new(AssignmentStrategy::RoundRobin);
        let agents: Vec<String> = vec![];
        let counts = HashMap::new();
        assert_eq!(assigner.pick_assignee(&agents, &counts), None);

        let mut assigner2 = TaskAssigner::new(AssignmentStrategy::LeastBusy);
        assert_eq!(assigner2.pick_assignee(&agents, &counts), None);
    }

    #[test]
    fn test_strategy_serialization() {
        let strategies = [
            AssignmentStrategy::Manual,
            AssignmentStrategy::RoundRobin,
            AssignmentStrategy::LeastBusy,
        ];
        for strategy in &strategies {
            let json = serde_json::to_string(strategy).unwrap();
            let deserialized: AssignmentStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(*strategy, deserialized);
        }

        // Verify snake_case naming
        assert_eq!(
            serde_json::to_string(&AssignmentStrategy::Manual).unwrap(),
            "\"manual\""
        );
        assert_eq!(
            serde_json::to_string(&AssignmentStrategy::RoundRobin).unwrap(),
            "\"round_robin\""
        );
        assert_eq!(
            serde_json::to_string(&AssignmentStrategy::LeastBusy).unwrap(),
            "\"least_busy\""
        );
    }
}
