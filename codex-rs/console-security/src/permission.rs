use serde::Deserialize;
use serde::Serialize;

/// Permission modes matching Claude Code behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Delegate,
    DontAsk,
    BypassPermissions,
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// Permission decision for a single action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

/// A single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Pattern matching the action (e.g., "file:write:*", "tool:exec:*").
    pub action_pattern: String,
    /// The decision for this rule.
    pub decision: PermissionDecision,
    /// Optional reason for the rule.
    pub reason: Option<String>,
}

/// A complete permission policy with ordered rules.
/// Rules are evaluated in order; first match wins.
/// Precedence: Deny > Ask > Allow (when multiple rules match at same level).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionPolicy {
    pub mode: PermissionMode,
    pub rules: Vec<PermissionRule>,
}

impl PermissionPolicy {
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode,
            rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: PermissionRule) {
        self.rules.push(rule);
    }

    /// Evaluate an action against the policy.
    /// Returns the decision based on mode defaults and matching rules.
    pub fn evaluate(&self, action: &str) -> PermissionDecision {
        // Check explicit rules first (first match wins with deny > ask > allow precedence)
        let mut result: Option<PermissionDecision> = None;
        for rule in &self.rules {
            if action_matches(&rule.action_pattern, action) {
                match (&result, &rule.decision) {
                    (None, _) => result = Some(rule.decision),
                    (Some(PermissionDecision::Allow), PermissionDecision::Deny) => {
                        result = Some(PermissionDecision::Deny);
                    }
                    (Some(PermissionDecision::Allow), PermissionDecision::Ask) => {
                        result = Some(PermissionDecision::Ask);
                    }
                    (Some(PermissionDecision::Ask), PermissionDecision::Deny) => {
                        result = Some(PermissionDecision::Deny);
                    }
                    _ => {} // Keep existing higher-precedence decision
                }
            }
        }

        if let Some(decision) = result {
            return decision;
        }

        // Fall back to mode default
        match self.mode {
            PermissionMode::BypassPermissions => PermissionDecision::Allow,
            PermissionMode::DontAsk => PermissionDecision::Allow,
            PermissionMode::AcceptEdits => PermissionDecision::Allow,
            PermissionMode::Plan => PermissionDecision::Deny,
            PermissionMode::Default => PermissionDecision::Ask,
            PermissionMode::Delegate => PermissionDecision::Ask,
        }
    }
}

/// Simple glob-like pattern matching for action patterns.
fn action_matches(pattern: &str, action: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return action.starts_with(prefix);
    }
    pattern == action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode_returns_ask() {
        let policy = PermissionPolicy::new(PermissionMode::Default);
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Ask
        );
    }

    #[test]
    fn test_bypass_mode_returns_allow() {
        let policy = PermissionPolicy::new(PermissionMode::BypassPermissions);
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn test_plan_mode_returns_deny() {
        let policy = PermissionPolicy::new(PermissionMode::Plan);
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Deny
        );
    }

    #[test]
    fn test_dont_ask_mode_returns_allow() {
        let policy = PermissionPolicy::new(PermissionMode::DontAsk);
        assert_eq!(policy.evaluate("tool:exec:ls"), PermissionDecision::Allow);
    }

    #[test]
    fn test_accept_edits_mode_returns_allow() {
        let policy = PermissionPolicy::new(PermissionMode::AcceptEdits);
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn test_delegate_mode_returns_ask() {
        let policy = PermissionPolicy::new(PermissionMode::Delegate);
        assert_eq!(policy.evaluate("tool:exec:rm"), PermissionDecision::Ask);
    }

    #[test]
    fn test_exact_rule_match() {
        let mut policy = PermissionPolicy::new(PermissionMode::Default);
        policy.add_rule(PermissionRule {
            action_pattern: "file:write:foo.rs".into(),
            decision: PermissionDecision::Allow,
            reason: None,
        });
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Allow
        );
        // Non-matching action falls back to mode default
        assert_eq!(
            policy.evaluate("file:write:bar.rs"),
            PermissionDecision::Ask
        );
    }

    #[test]
    fn test_glob_pattern_match() {
        let mut policy = PermissionPolicy::new(PermissionMode::Default);
        policy.add_rule(PermissionRule {
            action_pattern: "file:write:*".into(),
            decision: PermissionDecision::Allow,
            reason: None,
        });
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Allow
        );
        assert_eq!(
            policy.evaluate("file:write:bar.rs"),
            PermissionDecision::Allow
        );
        // Different prefix does not match
        assert_eq!(policy.evaluate("tool:exec:ls"), PermissionDecision::Ask);
    }

    #[test]
    fn test_wildcard_pattern_matches_everything() {
        let mut policy = PermissionPolicy::new(PermissionMode::Default);
        policy.add_rule(PermissionRule {
            action_pattern: "*".into(),
            decision: PermissionDecision::Deny,
            reason: Some("lockdown".into()),
        });
        assert_eq!(
            policy.evaluate("file:write:foo.rs"),
            PermissionDecision::Deny
        );
        assert_eq!(policy.evaluate("tool:exec:ls"), PermissionDecision::Deny);
    }

    #[test]
    fn test_deny_overrides_allow_precedence() {
        let mut policy = PermissionPolicy::new(PermissionMode::Default);
        policy.add_rule(PermissionRule {
            action_pattern: "file:write:*".into(),
            decision: PermissionDecision::Allow,
            reason: None,
        });
        policy.add_rule(PermissionRule {
            action_pattern: "file:write:secret*".into(),
            decision: PermissionDecision::Deny,
            reason: Some("sensitive".into()),
        });
        // "file:write:secret.txt" matches both rules; deny wins
        assert_eq!(
            policy.evaluate("file:write:secret.txt"),
            PermissionDecision::Deny
        );
        // "file:write:readme.md" only matches the allow rule
        assert_eq!(
            policy.evaluate("file:write:readme.md"),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn test_ask_overrides_allow_precedence() {
        let mut policy = PermissionPolicy::new(PermissionMode::BypassPermissions);
        policy.add_rule(PermissionRule {
            action_pattern: "tool:*".into(),
            decision: PermissionDecision::Allow,
            reason: None,
        });
        policy.add_rule(PermissionRule {
            action_pattern: "tool:exec:*".into(),
            decision: PermissionDecision::Ask,
            reason: Some("confirm exec".into()),
        });
        assert_eq!(policy.evaluate("tool:exec:rm"), PermissionDecision::Ask);
        assert_eq!(policy.evaluate("tool:read:file"), PermissionDecision::Allow);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut policy = PermissionPolicy::new(PermissionMode::AcceptEdits);
        policy.add_rule(PermissionRule {
            action_pattern: "file:write:*".into(),
            decision: PermissionDecision::Allow,
            reason: Some("auto-accept edits".into()),
        });
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: PermissionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.mode, PermissionMode::AcceptEdits);
        assert_eq!(deserialized.rules.len(), 1);
        assert_eq!(deserialized.rules[0].action_pattern, "file:write:*");
        assert_eq!(deserialized.rules[0].decision, PermissionDecision::Allow);
    }

    #[test]
    fn test_permission_mode_default() {
        let mode = PermissionMode::default();
        assert_eq!(mode, PermissionMode::Default);
    }

    #[test]
    fn test_empty_policy_uses_mode_default() {
        let policy = PermissionPolicy::default();
        assert_eq!(policy.mode, PermissionMode::Default);
        assert!(policy.rules.is_empty());
        assert_eq!(policy.evaluate("anything"), PermissionDecision::Ask);
    }
}
