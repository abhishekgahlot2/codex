use serde::{Deserialize, Serialize};

/// Events that can trigger hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    SessionStart,
    PreToolUse,
    PostToolUse,
    Stop,
    SessionEnd,
    TeamCreated,
    TaskCompleted,
    TeammateIdle,
}

/// Specification for a registered hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    pub name: String,
    pub event: HookEvent,
    pub command: String,
    pub timeout_ms: u64,
    pub enabled: bool,
}

impl HookSpec {
    pub fn new(name: &str, event: HookEvent, command: &str) -> Self {
        Self {
            name: name.into(),
            event,
            command: command.into(),
            timeout_ms: 5000,
            enabled: true,
        }
    }
}

/// Result of a hook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    pub hook_name: String,
    pub event: HookEvent,
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Hook gate decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Allow,
    Block,
    Warn,
}

/// Manages registered hooks.
#[derive(Debug, Clone, Default)]
pub struct HookRegistry {
    hooks: Vec<HookSpec>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    pub fn register(&mut self, spec: HookSpec) {
        self.hooks.push(spec);
    }

    pub fn hooks_for_event(&self, event: HookEvent) -> Vec<&HookSpec> {
        self.hooks
            .iter()
            .filter(|h| h.event == event && h.enabled)
            .collect()
    }

    pub fn all_hooks(&self) -> &[HookSpec] {
        &self.hooks
    }

    pub fn enable(&mut self, name: &str) -> bool {
        if let Some(h) = self.hooks.iter_mut().find(|h| h.name == name) {
            h.enabled = true;
            true
        } else {
            false
        }
    }

    pub fn disable(&mut self, name: &str) -> bool {
        if let Some(h) = self.hooks.iter_mut().find(|h| h.name == name) {
            h.enabled = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_lookup() {
        let mut registry = HookRegistry::new();
        registry.register(HookSpec::new("pre-lint", HookEvent::PreToolUse, "lint.sh"));
        registry.register(HookSpec::new("post-fmt", HookEvent::PostToolUse, "fmt.sh"));

        assert_eq!(registry.all_hooks().len(), 2);
    }

    #[test]
    fn test_hooks_for_event_filtering() {
        let mut registry = HookRegistry::new();
        registry.register(HookSpec::new("a", HookEvent::PreToolUse, "a.sh"));
        registry.register(HookSpec::new("b", HookEvent::PostToolUse, "b.sh"));
        registry.register(HookSpec::new("c", HookEvent::PreToolUse, "c.sh"));

        let pre = registry.hooks_for_event(HookEvent::PreToolUse);
        assert_eq!(pre.len(), 2);
        assert!(pre.iter().all(|h| h.event == HookEvent::PreToolUse));

        let post = registry.hooks_for_event(HookEvent::PostToolUse);
        assert_eq!(post.len(), 1);
    }

    #[test]
    fn test_enable_disable() {
        let mut registry = HookRegistry::new();
        registry.register(HookSpec::new("hook1", HookEvent::Stop, "stop.sh"));

        assert!(registry.disable("hook1"));
        assert!(registry.hooks_for_event(HookEvent::Stop).is_empty());

        assert!(registry.enable("hook1"));
        assert_eq!(registry.hooks_for_event(HookEvent::Stop).len(), 1);

        // Non-existent hook returns false.
        assert!(!registry.enable("no-such-hook"));
        assert!(!registry.disable("no-such-hook"));
    }

    #[test]
    fn test_hook_event_serialization() {
        let json = serde_json::to_string(&HookEvent::SessionStart).unwrap();
        assert_eq!(json, "\"session_start\"");

        let parsed: HookEvent = serde_json::from_str("\"teammate_idle\"").unwrap();
        assert_eq!(parsed, HookEvent::TeammateIdle);
    }

    #[test]
    fn test_hookspec_new_defaults() {
        let spec = HookSpec::new("test", HookEvent::SessionEnd, "end.sh");
        assert_eq!(spec.timeout_ms, 5000);
        assert!(spec.enabled);
        assert_eq!(spec.name, "test");
        assert_eq!(spec.command, "end.sh");
        assert_eq!(spec.event, HookEvent::SessionEnd);
    }
}
