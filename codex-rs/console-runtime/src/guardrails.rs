use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// ToolBudget — Limits on tool calls
// ---------------------------------------------------------------------------

/// Configurable budget limiting tool calls per turn and per session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBudget {
    /// Maximum tool calls allowed in a single turn. 0 = unlimited.
    pub max_per_turn: u32,
    /// Maximum tool calls allowed across the entire session. 0 = unlimited.
    pub max_per_session: u32,
}

impl Default for ToolBudget {
    fn default() -> Self {
        Self {
            max_per_turn: 50,
            max_per_session: 500,
        }
    }
}

// ---------------------------------------------------------------------------
// GuardrailViolation — Error type
// ---------------------------------------------------------------------------

/// A guardrail violation that should stop or warn about tool execution.
#[derive(Debug, Clone)]
pub enum GuardrailViolation {
    TurnBudgetExceeded { limit: u32, count: u32 },
    SessionBudgetExceeded { limit: u32, count: u32 },
    LoopDetected { tool_name: String, occurrences: usize, window: usize },
    TurnTimeout { elapsed: Duration, limit: Duration },
}

impl std::fmt::Display for GuardrailViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TurnBudgetExceeded { limit, count } => {
                write!(f, "turn tool budget exceeded: {count}/{limit} calls")
            }
            Self::SessionBudgetExceeded { limit, count } => {
                write!(f, "session tool budget exceeded: {count}/{limit} calls")
            }
            Self::LoopDetected {
                tool_name,
                occurrences,
                window,
            } => {
                write!(
                    f,
                    "loop detected: '{tool_name}' called {occurrences} times in last {window} calls"
                )
            }
            Self::TurnTimeout { elapsed, limit } => {
                write!(f, "turn timeout: {elapsed:?} exceeded limit of {limit:?}")
            }
        }
    }
}

impl std::error::Error for GuardrailViolation {}

// ---------------------------------------------------------------------------
// ToolBudgetTracker — Tracks tool call counts against a budget
// ---------------------------------------------------------------------------

/// Tracks tool call counts against a budget.
#[derive(Debug, Clone)]
pub struct ToolBudgetTracker {
    budget: ToolBudget,
    turn_count: u32,
    session_count: u32,
}

impl ToolBudgetTracker {
    pub fn new(budget: ToolBudget) -> Self {
        Self {
            budget,
            turn_count: 0,
            session_count: 0,
        }
    }

    /// Record a tool call. Returns Err with violation if budget exceeded.
    pub fn record_call(&mut self) -> Result<(), GuardrailViolation> {
        self.turn_count += 1;
        self.session_count += 1;
        if self.budget.max_per_turn > 0 && self.turn_count > self.budget.max_per_turn {
            return Err(GuardrailViolation::TurnBudgetExceeded {
                limit: self.budget.max_per_turn,
                count: self.turn_count,
            });
        }
        if self.budget.max_per_session > 0 && self.session_count > self.budget.max_per_session {
            return Err(GuardrailViolation::SessionBudgetExceeded {
                limit: self.budget.max_per_session,
                count: self.session_count,
            });
        }
        Ok(())
    }

    /// Reset turn counter (call at start of each new turn).
    pub fn reset_turn(&mut self) {
        self.turn_count = 0;
    }

    /// Current turn count.
    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    /// Current session count.
    pub fn session_count(&self) -> u32 {
        self.session_count
    }
}

// ---------------------------------------------------------------------------
// LoopDetector — Detects repeated tool call patterns
// ---------------------------------------------------------------------------

/// Detects when the model is stuck in a loop calling the same tools repeatedly.
#[derive(Debug, Clone)]
pub struct LoopDetector {
    /// Window size: number of recent calls to track.
    window_size: usize,
    /// Threshold: if the same tool is called this many times in the window, flag it.
    repeat_threshold: usize,
    /// Recent tool call history (tool names).
    history: Vec<String>,
}

impl LoopDetector {
    pub fn new(window_size: usize, repeat_threshold: usize) -> Self {
        Self {
            window_size,
            repeat_threshold,
            history: Vec::with_capacity(window_size),
        }
    }

    /// Record a tool call and check for loops.
    pub fn record_and_check(&mut self, tool_name: &str) -> Result<(), GuardrailViolation> {
        self.history.push(tool_name.to_string());
        if self.history.len() > self.window_size {
            self.history.remove(0);
        }
        // Count occurrences of this tool in the window
        let count = self.history.iter().filter(|n| n.as_str() == tool_name).count();
        if count >= self.repeat_threshold {
            return Err(GuardrailViolation::LoopDetected {
                tool_name: tool_name.to_string(),
                occurrences: count,
                window: self.window_size,
            });
        }
        Ok(())
    }

    /// Reset detector (call at start of each new turn).
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new(10, 5) // 5 calls to same tool in last 10 = loop
    }
}

// ---------------------------------------------------------------------------
// TurnTimeout — Configurable turn duration limit
// ---------------------------------------------------------------------------

/// Enforces a maximum duration for a single turn.
#[derive(Debug, Clone)]
pub struct TurnTimeout {
    max_duration: Duration,
    started_at: Option<Instant>,
}

impl TurnTimeout {
    pub fn new(max_duration: Duration) -> Self {
        Self {
            max_duration,
            started_at: None,
        }
    }

    /// Start the timer for a new turn.
    pub fn start(&mut self) {
        self.started_at = Some(Instant::now());
    }

    /// Check if the turn has exceeded its time limit.
    pub fn check(&self) -> Result<(), GuardrailViolation> {
        if let Some(started) = self.started_at {
            let elapsed = started.elapsed();
            if elapsed > self.max_duration {
                return Err(GuardrailViolation::TurnTimeout {
                    elapsed,
                    limit: self.max_duration,
                });
            }
        }
        Ok(())
    }

    /// Elapsed time since turn start, if started.
    pub fn elapsed(&self) -> Option<Duration> {
        self.started_at.map(|s| s.elapsed())
    }

    /// Reset (call at start of each new turn).
    pub fn reset(&mut self) {
        self.started_at = None;
    }
}

impl Default for TurnTimeout {
    fn default() -> Self {
        Self::new(Duration::from_secs(300)) // 5 minutes per turn
    }
}

// ---------------------------------------------------------------------------
// GuardrailSet — Combined guardrail checker
// ---------------------------------------------------------------------------

/// Combines all guardrails into a single checker.
#[derive(Debug, Clone)]
pub struct GuardrailSet {
    pub budget: ToolBudgetTracker,
    pub loop_detector: LoopDetector,
    pub timeout: TurnTimeout,
}

impl GuardrailSet {
    pub fn new(budget: ToolBudget) -> Self {
        Self {
            budget: ToolBudgetTracker::new(budget),
            loop_detector: LoopDetector::default(),
            timeout: TurnTimeout::default(),
        }
    }

    /// Check all guardrails before executing a tool call.
    pub fn check_before_call(&mut self, tool_name: &str) -> Result<(), GuardrailViolation> {
        self.timeout.check()?;
        self.budget.record_call()?;
        self.loop_detector.record_and_check(tool_name)?;
        Ok(())
    }

    /// Reset per-turn state (call at start of each new turn).
    pub fn reset_turn(&mut self) {
        self.budget.reset_turn();
        self.loop_detector.reset();
        self.timeout.reset();
    }

    /// Start the turn timer.
    pub fn start_turn(&mut self) {
        self.timeout.start();
    }
}

impl Default for GuardrailSet {
    fn default() -> Self {
        Self::new(ToolBudget::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_tracker_within_limit() {
        let budget = ToolBudget {
            max_per_turn: 5,
            max_per_session: 10,
        };
        let mut tracker = ToolBudgetTracker::new(budget);
        for _ in 0..5 {
            assert!(tracker.record_call().is_ok());
        }
        assert_eq!(tracker.turn_count(), 5);
        assert_eq!(tracker.session_count(), 5);
    }

    #[test]
    fn test_budget_turn_exceeded() {
        let budget = ToolBudget {
            max_per_turn: 3,
            max_per_session: 100,
        };
        let mut tracker = ToolBudgetTracker::new(budget);
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        let result = tracker.record_call();
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            GuardrailViolation::TurnBudgetExceeded { limit, count } => {
                assert_eq!(limit, 3);
                assert_eq!(count, 4);
            }
            other => panic!("expected TurnBudgetExceeded, got {other:?}"),
        }
    }

    #[test]
    fn test_budget_session_exceeded() {
        let budget = ToolBudget {
            max_per_turn: 0, // unlimited per turn
            max_per_session: 3,
        };
        let mut tracker = ToolBudgetTracker::new(budget);
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        let result = tracker.record_call();
        assert!(result.is_err());
        match result.unwrap_err() {
            GuardrailViolation::SessionBudgetExceeded { limit, count } => {
                assert_eq!(limit, 3);
                assert_eq!(count, 4);
            }
            other => panic!("expected SessionBudgetExceeded, got {other:?}"),
        }
    }

    #[test]
    fn test_budget_reset_turn() {
        let budget = ToolBudget {
            max_per_turn: 2,
            max_per_session: 100,
        };
        let mut tracker = ToolBudgetTracker::new(budget);
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        // Turn limit reached, next call would fail
        assert!(tracker.record_call().is_err());

        // Reset turn — turn count goes to 0, but session count stays
        tracker.reset_turn();
        assert_eq!(tracker.turn_count(), 0);
        assert_eq!(tracker.session_count(), 3);

        // Can make calls again within the new turn
        assert!(tracker.record_call().is_ok());
        assert!(tracker.record_call().is_ok());
        assert_eq!(tracker.turn_count(), 2);
        assert_eq!(tracker.session_count(), 5);
    }

    #[test]
    fn test_budget_unlimited() {
        let budget = ToolBudget {
            max_per_turn: 0,
            max_per_session: 0,
        };
        let mut tracker = ToolBudgetTracker::new(budget);
        // Should allow many calls when both limits are 0 (unlimited)
        for _ in 0..1000 {
            assert!(tracker.record_call().is_ok());
        }
    }

    #[test]
    fn test_loop_detector_no_loop() {
        let mut detector = LoopDetector::new(10, 5);
        assert!(detector.record_and_check("read").is_ok());
        assert!(detector.record_and_check("write").is_ok());
        assert!(detector.record_and_check("grep").is_ok());
        assert!(detector.record_and_check("edit").is_ok());
        assert!(detector.record_and_check("glob").is_ok());
    }

    #[test]
    fn test_loop_detector_detects_loop() {
        let mut detector = LoopDetector::new(10, 5);
        // Call the same tool 5 times in a window of 10
        assert!(detector.record_and_check("read").is_ok());
        assert!(detector.record_and_check("read").is_ok());
        assert!(detector.record_and_check("read").is_ok());
        assert!(detector.record_and_check("read").is_ok());
        let result = detector.record_and_check("read");
        assert!(result.is_err());
        match result.unwrap_err() {
            GuardrailViolation::LoopDetected {
                tool_name,
                occurrences,
                window,
            } => {
                assert_eq!(tool_name, "read");
                assert_eq!(occurrences, 5);
                assert_eq!(window, 10);
            }
            other => panic!("expected LoopDetected, got {other:?}"),
        }
    }

    #[test]
    fn test_loop_detector_reset() {
        let mut detector = LoopDetector::new(10, 5);
        for _ in 0..4 {
            assert!(detector.record_and_check("read").is_ok());
        }
        detector.reset();
        // After reset, the history is cleared so the same tool can be called again
        for _ in 0..4 {
            assert!(detector.record_and_check("read").is_ok());
        }
    }

    #[test]
    fn test_turn_timeout_within_limit() {
        let mut timeout = TurnTimeout::new(Duration::from_secs(60));
        timeout.start();
        // Immediately check — should be well within limit
        assert!(timeout.check().is_ok());
        assert!(timeout.elapsed().is_some());
    }

    #[test]
    fn test_turn_timeout_not_started() {
        let timeout = TurnTimeout::new(Duration::from_secs(1));
        // Check without starting — should pass (no timer running)
        assert!(timeout.check().is_ok());
        assert!(timeout.elapsed().is_none());
    }

    #[test]
    fn test_guardrail_set_combined() {
        let budget = ToolBudget {
            max_per_turn: 10,
            max_per_session: 100,
        };
        let mut set = GuardrailSet::new(budget);
        set.start_turn();

        // Normal calls should work
        assert!(set.check_before_call("read").is_ok());
        assert!(set.check_before_call("write").is_ok());
        assert!(set.check_before_call("grep").is_ok());

        // Reset turn clears per-turn state
        set.reset_turn();
        set.start_turn();
        assert!(set.check_before_call("edit").is_ok());
    }

    #[test]
    fn test_guardrail_violation_display() {
        let v1 = GuardrailViolation::TurnBudgetExceeded {
            limit: 50,
            count: 51,
        };
        assert_eq!(v1.to_string(), "turn tool budget exceeded: 51/50 calls");

        let v2 = GuardrailViolation::SessionBudgetExceeded {
            limit: 500,
            count: 501,
        };
        assert_eq!(
            v2.to_string(),
            "session tool budget exceeded: 501/500 calls"
        );

        let v3 = GuardrailViolation::LoopDetected {
            tool_name: "read".to_string(),
            occurrences: 5,
            window: 10,
        };
        assert_eq!(
            v3.to_string(),
            "loop detected: 'read' called 5 times in last 10 calls"
        );

        let v4 = GuardrailViolation::TurnTimeout {
            elapsed: Duration::from_secs(301),
            limit: Duration::from_secs(300),
        };
        let display = v4.to_string();
        assert!(display.contains("turn timeout:"));
        assert!(display.contains("exceeded limit of"));
    }
}
