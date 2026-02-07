use serde::{Deserialize, Serialize};

/// Execution modes that affect tool availability and behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    /// Default execution mode. All tools available, full autonomy.
    Build,
    /// Planning mode. Read-only tools only, no mutations.
    Plan,
    /// Review mode. Analysis tools available, limited mutations.
    Review,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Build
    }
}

/// Policy effects for an execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModePolicy {
    /// The mode this policy applies to.
    pub mode: ExecutionMode,
    /// Tool name prefixes that are allowed in this mode.
    /// Empty = all tools allowed.
    pub allowed_tool_prefixes: Vec<String>,
    /// Tool name prefixes that are blocked in this mode.
    /// Checked after allowed (blocklist takes priority).
    pub blocked_tool_prefixes: Vec<String>,
    /// Whether file-mutating operations are allowed.
    pub allow_mutations: bool,
    /// Whether network operations are allowed.
    pub allow_network: bool,
    /// Human-readable description of this mode.
    pub description: String,
}

impl ModePolicy {
    /// Check if a tool is allowed under this mode's policy.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Blocklist takes priority
        for prefix in &self.blocked_tool_prefixes {
            if tool_name.starts_with(prefix.as_str()) {
                return false;
            }
        }
        // If allowlist is empty, everything is allowed
        if self.allowed_tool_prefixes.is_empty() {
            return true;
        }
        // Otherwise, must match an allow prefix
        self.allowed_tool_prefixes
            .iter()
            .any(|p| tool_name.starts_with(p.as_str()))
    }
}

/// Returns the default mode policies for all execution modes.
pub fn default_mode_policies() -> Vec<ModePolicy> {
    vec![
        ModePolicy {
            mode: ExecutionMode::Build,
            allowed_tool_prefixes: vec![], // All tools allowed
            blocked_tool_prefixes: vec![],
            allow_mutations: true,
            allow_network: true,
            description: "Full execution mode. All tools available with full autonomy.".into(),
        },
        ModePolicy {
            mode: ExecutionMode::Plan,
            allowed_tool_prefixes: vec![
                "read".into(),
                "grep".into(),
                "glob".into(),
                "search".into(),
                "list".into(),
                "team_status".into(),
                "team_list".into(),
            ],
            blocked_tool_prefixes: vec![
                "write".into(),
                "edit".into(),
                "delete".into(),
                "exec".into(),
            ],
            allow_mutations: false,
            allow_network: false,
            description: "Planning mode. Read-only tools, no file mutations or network.".into(),
        },
        ModePolicy {
            mode: ExecutionMode::Review,
            allowed_tool_prefixes: vec![], // All tools allowed
            blocked_tool_prefixes: vec!["delete".into(), "exec".into()],
            allow_mutations: false,
            allow_network: true,
            description: "Review mode. Analysis tools with read access, no destructive operations."
                .into(),
        },
    ]
}

/// Finds the policy for a given execution mode from a list of policies.
pub fn policy_for_mode(policies: &[ModePolicy], mode: ExecutionMode) -> Option<&ModePolicy> {
    policies.iter().find(|p| p.mode == mode)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode_is_build() {
        assert_eq!(ExecutionMode::default(), ExecutionMode::Build);
    }

    #[test]
    fn test_build_mode_allows_all_tools() {
        let policies = default_mode_policies();
        let build = policy_for_mode(&policies, ExecutionMode::Build).unwrap();
        assert!(build.is_tool_allowed("read"));
        assert!(build.is_tool_allowed("write"));
        assert!(build.is_tool_allowed("edit"));
        assert!(build.is_tool_allowed("delete"));
        assert!(build.is_tool_allowed("exec"));
        assert!(build.is_tool_allowed("anything_else"));
    }

    #[test]
    fn test_plan_mode_blocks_mutations() {
        let policies = default_mode_policies();
        let plan = policy_for_mode(&policies, ExecutionMode::Plan).unwrap();
        assert!(!plan.is_tool_allowed("write_file"));
        assert!(!plan.is_tool_allowed("edit_file"));
        assert!(!plan.is_tool_allowed("delete_file"));
        assert!(!plan.is_tool_allowed("exec_command"));
        assert!(!plan.allow_mutations);
    }

    #[test]
    fn test_plan_mode_allows_reads() {
        let policies = default_mode_policies();
        let plan = policy_for_mode(&policies, ExecutionMode::Plan).unwrap();
        assert!(plan.is_tool_allowed("read_file"));
        assert!(plan.is_tool_allowed("grep_search"));
        assert!(plan.is_tool_allowed("glob_pattern"));
        assert!(plan.is_tool_allowed("search_code"));
        assert!(plan.is_tool_allowed("list_files"));
    }

    #[test]
    fn test_review_mode_blocks_destructive() {
        let policies = default_mode_policies();
        let review = policy_for_mode(&policies, ExecutionMode::Review).unwrap();
        assert!(!review.is_tool_allowed("delete_file"));
        assert!(!review.is_tool_allowed("exec_command"));
    }

    #[test]
    fn test_review_mode_allows_read() {
        let policies = default_mode_policies();
        let review = policy_for_mode(&policies, ExecutionMode::Review).unwrap();
        assert!(review.is_tool_allowed("read_file"));
        assert!(review.is_tool_allowed("grep_search"));
        assert!(review.is_tool_allowed("write_file")); // review allows write (not in blocklist)
        assert!(review.allow_network);
    }

    #[test]
    fn test_blocklist_priority() {
        // Create a policy where a tool matches both allow and block
        let policy = ModePolicy {
            mode: ExecutionMode::Plan,
            allowed_tool_prefixes: vec!["read".into()],
            blocked_tool_prefixes: vec!["read_secret".into()],
            allow_mutations: false,
            allow_network: false,
            description: "test".into(),
        };
        // "read_file" matches allow and not block => allowed
        assert!(policy.is_tool_allowed("read_file"));
        // "read_secret_data" matches both allow prefix "read" and block prefix "read_secret" => blocked
        assert!(!policy.is_tool_allowed("read_secret_data"));
    }

    #[test]
    fn test_mode_serialization_roundtrip() {
        let mode = ExecutionMode::Plan;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"plan\"");
        let deserialized: ExecutionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);

        let build_json = serde_json::to_string(&ExecutionMode::Build).unwrap();
        assert_eq!(build_json, "\"build\"");

        let review_json = serde_json::to_string(&ExecutionMode::Review).unwrap();
        assert_eq!(review_json, "\"review\"");
    }

    #[test]
    fn test_default_policies_cover_all_modes() {
        let policies = default_mode_policies();
        assert!(policy_for_mode(&policies, ExecutionMode::Build).is_some());
        assert!(policy_for_mode(&policies, ExecutionMode::Plan).is_some());
        assert!(policy_for_mode(&policies, ExecutionMode::Review).is_some());
    }

    #[test]
    fn test_policy_for_mode_lookup() {
        let policies = default_mode_policies();
        let build = policy_for_mode(&policies, ExecutionMode::Build).unwrap();
        assert_eq!(build.mode, ExecutionMode::Build);
        assert!(build.allow_mutations);

        let plan = policy_for_mode(&policies, ExecutionMode::Plan).unwrap();
        assert_eq!(plan.mode, ExecutionMode::Plan);
        assert!(!plan.allow_mutations);

        // Lookup for a mode not in the list returns None
        let empty: Vec<ModePolicy> = vec![];
        assert!(policy_for_mode(&empty, ExecutionMode::Build).is_none());
    }
}
