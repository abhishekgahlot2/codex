use serde::Deserialize;
use serde::Serialize;

/// Phases of the tool execution loop: Plan -> Act -> Observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLoopPhase {
    /// Planning: model is reasoning about what to do next.
    Plan,
    /// Acting: tools are being called.
    Act,
    /// Observing: results are being analyzed.
    Observe,
}

/// Tracks the state of the tool execution loop within a turn.
#[derive(Debug, Clone)]
pub struct LoopState {
    /// Current phase.
    pub phase: ToolLoopPhase,
    /// Number of plan->act->observe cycles completed this turn.
    pub cycle_count: u32,
    /// Total tool calls executed in current cycle.
    pub calls_in_cycle: u32,
    /// Tool names called in current cycle.
    pub tools_in_cycle: Vec<String>,
}

impl LoopState {
    pub fn new() -> Self {
        Self {
            phase: ToolLoopPhase::Plan,
            cycle_count: 0,
            calls_in_cycle: 0,
            tools_in_cycle: Vec::new(),
        }
    }

    /// Transition to the next phase.
    pub fn advance_phase(&mut self) {
        self.phase = match self.phase {
            ToolLoopPhase::Plan => ToolLoopPhase::Act,
            ToolLoopPhase::Act => ToolLoopPhase::Observe,
            ToolLoopPhase::Observe => {
                self.cycle_count += 1;
                self.calls_in_cycle = 0;
                self.tools_in_cycle.clear();
                ToolLoopPhase::Plan
            }
        };
    }

    /// Record a tool call during the Act phase.
    pub fn record_tool_call(&mut self, tool_name: &str) {
        self.calls_in_cycle += 1;
        self.tools_in_cycle.push(tool_name.to_string());
    }

    /// Reset to initial state (call at start of each new turn).
    pub fn reset(&mut self) {
        self.phase = ToolLoopPhase::Plan;
        self.cycle_count = 0;
        self.calls_in_cycle = 0;
        self.tools_in_cycle.clear();
    }
}

impl Default for LoopState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = LoopState::new();
        assert_eq!(state.phase, ToolLoopPhase::Plan);
        assert_eq!(state.cycle_count, 0);
        assert_eq!(state.calls_in_cycle, 0);
        assert!(state.tools_in_cycle.is_empty());
    }

    #[test]
    fn test_phase_transitions() {
        let mut state = LoopState::new();

        // Plan -> Act
        assert_eq!(state.phase, ToolLoopPhase::Plan);
        state.advance_phase();
        assert_eq!(state.phase, ToolLoopPhase::Act);

        // Act -> Observe
        state.advance_phase();
        assert_eq!(state.phase, ToolLoopPhase::Observe);

        // Observe -> Plan (completes one cycle)
        state.advance_phase();
        assert_eq!(state.phase, ToolLoopPhase::Plan);
    }

    #[test]
    fn test_cycle_count_increments() {
        let mut state = LoopState::new();
        assert_eq!(state.cycle_count, 0);

        // Complete one full cycle: Plan -> Act -> Observe -> Plan
        state.advance_phase(); // -> Act
        state.advance_phase(); // -> Observe
        state.advance_phase(); // -> Plan (cycle_count becomes 1)
        assert_eq!(state.cycle_count, 1);

        // Another cycle
        state.advance_phase(); // -> Act
        state.advance_phase(); // -> Observe
        state.advance_phase(); // -> Plan (cycle_count becomes 2)
        assert_eq!(state.cycle_count, 2);
    }

    #[test]
    fn test_tool_call_recording() {
        let mut state = LoopState::new();
        state.advance_phase(); // -> Act

        state.record_tool_call("read");
        state.record_tool_call("write");
        state.record_tool_call("grep");

        assert_eq!(state.calls_in_cycle, 3);
        assert_eq!(state.tools_in_cycle, vec!["read", "write", "grep"]);
    }

    #[test]
    fn test_reset() {
        let mut state = LoopState::new();

        // Advance and record some state
        state.advance_phase(); // -> Act
        state.record_tool_call("read");
        state.record_tool_call("write");
        state.advance_phase(); // -> Observe
        state.advance_phase(); // -> Plan (cycle 1)

        assert_eq!(state.cycle_count, 1);

        // Reset
        state.reset();
        assert_eq!(state.phase, ToolLoopPhase::Plan);
        assert_eq!(state.cycle_count, 0);
        assert_eq!(state.calls_in_cycle, 0);
        assert!(state.tools_in_cycle.is_empty());
    }

    #[test]
    fn test_multiple_cycles() {
        let mut state = LoopState::new();

        for expected_cycle in 1..=3 {
            // Plan -> Act
            state.advance_phase();
            assert_eq!(state.phase, ToolLoopPhase::Act);

            // Record tool calls during Act
            state.record_tool_call("tool_a");
            state.record_tool_call("tool_b");
            assert_eq!(state.calls_in_cycle, 2);

            // Act -> Observe
            state.advance_phase();
            assert_eq!(state.phase, ToolLoopPhase::Observe);

            // Observe -> Plan (cycle completes, calls_in_cycle resets)
            state.advance_phase();
            assert_eq!(state.phase, ToolLoopPhase::Plan);
            assert_eq!(state.cycle_count, expected_cycle);
            assert_eq!(state.calls_in_cycle, 0);
            assert!(state.tools_in_cycle.is_empty());
        }
    }
}
