use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Plugin runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    Registered,
    Initializing,
    Running,
    Stopped,
    Error,
}

/// A lifecycle event for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleEvent {
    pub plugin_name: String,
    pub from_state: PluginState,
    pub to_state: PluginState,
    pub timestamp: String,
    pub error: Option<String>,
}

/// Tracks the lifecycle state of plugins.
#[derive(Debug, Clone, Default)]
pub struct LifecycleTracker {
    states: HashMap<String, PluginState>,
    events: Vec<LifecycleEvent>,
}

impl LifecycleTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transition(&mut self, plugin_name: &str, to_state: PluginState) {
        let from_state = self
            .states
            .get(plugin_name)
            .copied()
            .unwrap_or(PluginState::Registered);
        self.states.insert(plugin_name.to_string(), to_state);
        self.events.push(LifecycleEvent {
            plugin_name: plugin_name.into(),
            from_state,
            to_state,
            timestamp: String::new(),
            error: None,
        });
    }

    pub fn state_of(&self, plugin_name: &str) -> PluginState {
        self.states
            .get(plugin_name)
            .copied()
            .unwrap_or(PluginState::Registered)
    }

    pub fn events_for(&self, plugin_name: &str) -> Vec<&LifecycleEvent> {
        self.events
            .iter()
            .filter(|e| e.plugin_name == plugin_name)
            .collect()
    }

    pub fn running_plugins(&self) -> Vec<&str> {
        self.states
            .iter()
            .filter(|(_, s)| **s == PluginState::Running)
            .map(|(name, _)| name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let tracker = LifecycleTracker::new();
        assert_eq!(tracker.state_of("unknown"), PluginState::Registered);
    }

    #[test]
    fn test_transitions() {
        let mut tracker = LifecycleTracker::new();
        tracker.transition("p1", PluginState::Initializing);
        assert_eq!(tracker.state_of("p1"), PluginState::Initializing);

        tracker.transition("p1", PluginState::Running);
        assert_eq!(tracker.state_of("p1"), PluginState::Running);

        tracker.transition("p1", PluginState::Stopped);
        assert_eq!(tracker.state_of("p1"), PluginState::Stopped);
    }

    #[test]
    fn test_events_tracking() {
        let mut tracker = LifecycleTracker::new();
        tracker.transition("p1", PluginState::Initializing);
        tracker.transition("p1", PluginState::Running);
        tracker.transition("p2", PluginState::Error);

        let p1_events = tracker.events_for("p1");
        assert_eq!(p1_events.len(), 2);
        assert_eq!(p1_events[0].from_state, PluginState::Registered);
        assert_eq!(p1_events[0].to_state, PluginState::Initializing);
        assert_eq!(p1_events[1].from_state, PluginState::Initializing);
        assert_eq!(p1_events[1].to_state, PluginState::Running);

        let p2_events = tracker.events_for("p2");
        assert_eq!(p2_events.len(), 1);
    }

    #[test]
    fn test_running_plugins() {
        let mut tracker = LifecycleTracker::new();
        tracker.transition("a", PluginState::Running);
        tracker.transition("b", PluginState::Stopped);
        tracker.transition("c", PluginState::Running);

        let mut running = tracker.running_plugins();
        running.sort();
        assert_eq!(running, vec!["a", "c"]);
    }

    #[test]
    fn test_state_serialization() {
        let json = serde_json::to_string(&PluginState::Initializing).unwrap();
        assert_eq!(json, "\"initializing\"");

        let parsed: PluginState = serde_json::from_str("\"error\"").unwrap();
        assert_eq!(parsed, PluginState::Error);
    }
}
