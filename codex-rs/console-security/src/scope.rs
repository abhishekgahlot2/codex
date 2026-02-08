use serde::Deserialize;
use serde::Serialize;

/// Filesystem access scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemScope {
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
}

impl Default for FilesystemScope {
    fn default() -> Self {
        Self {
            allowed_paths: vec![".".into()],
            denied_paths: vec![
                "/etc".into(),
                "/var".into(),
                "/usr".into(),
                "~/.ssh".into(),
                "~/.gnupg".into(),
            ],
        }
    }
}

impl FilesystemScope {
    pub fn is_path_allowed(&self, path: &str) -> bool {
        // Check denied first
        for denied in &self.denied_paths {
            if path.starts_with(denied.as_str()) {
                return false;
            }
        }
        // Check allowed
        if self.allowed_paths.is_empty() {
            return true;
        }
        self.allowed_paths
            .iter()
            .any(|a| path.starts_with(a.as_str()))
    }
}

/// Command execution scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandScope {
    pub allowed_commands: Vec<String>,
    pub denied_commands: Vec<String>,
}

impl Default for CommandScope {
    fn default() -> Self {
        Self {
            allowed_commands: vec![], // Empty = all allowed
            denied_commands: vec![
                "rm -rf /".into(),
                "mkfs".into(),
                "dd".into(),
                ":(){ :|:& };:".into(),
            ],
        }
    }
}

impl CommandScope {
    pub fn is_command_allowed(&self, cmd: &str) -> bool {
        for denied in &self.denied_commands {
            if cmd.contains(denied.as_str()) {
                return false;
            }
        }
        if self.allowed_commands.is_empty() {
            return true;
        }
        self.allowed_commands
            .iter()
            .any(|a| cmd.starts_with(a.as_str()))
    }
}

/// Provider/API access scope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderScope {
    pub allowed_providers: Vec<String>,
    pub max_cost_per_session_usd: Option<f64>,
    pub max_tokens_per_session: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- FilesystemScope tests ---

    #[test]
    fn test_filesystem_scope_defaults() {
        let scope = FilesystemScope::default();
        assert_eq!(scope.allowed_paths, vec!["."]);
        assert!(scope.denied_paths.contains(&"/etc".to_string()));
        assert!(scope.denied_paths.contains(&"~/.ssh".to_string()));
    }

    #[test]
    fn test_filesystem_deny_takes_priority() {
        let scope = FilesystemScope::default();
        assert!(!scope.is_path_allowed("/etc/passwd"));
        assert!(!scope.is_path_allowed("/var/log/syslog"));
        assert!(!scope.is_path_allowed("/usr/bin/ls"));
        assert!(!scope.is_path_allowed("~/.ssh/id_rsa"));
        assert!(!scope.is_path_allowed("~/.gnupg/keys"));
    }

    #[test]
    fn test_filesystem_allowed_path() {
        let scope = FilesystemScope::default();
        assert!(scope.is_path_allowed("./src/main.rs"));
        assert!(scope.is_path_allowed("./Cargo.toml"));
    }

    #[test]
    fn test_filesystem_path_not_in_allowed() {
        let scope = FilesystemScope::default();
        // Path that is neither denied nor starts with "."
        assert!(!scope.is_path_allowed("/home/user/file.txt"));
    }

    #[test]
    fn test_filesystem_empty_allowed_permits_all() {
        let scope = FilesystemScope {
            allowed_paths: vec![],
            denied_paths: vec!["/secret".into()],
        };
        assert!(scope.is_path_allowed("/home/user/file.txt"));
        assert!(scope.is_path_allowed("/tmp/foo"));
        assert!(!scope.is_path_allowed("/secret/key"));
    }

    #[test]
    fn test_filesystem_custom_scope() {
        let scope = FilesystemScope {
            allowed_paths: vec!["/home/user/project".into()],
            denied_paths: vec!["/home/user/project/.env".into()],
        };
        assert!(scope.is_path_allowed("/home/user/project/src/main.rs"));
        assert!(!scope.is_path_allowed("/home/user/project/.env"));
        assert!(!scope.is_path_allowed("/tmp/foo"));
    }

    // --- CommandScope tests ---

    #[test]
    fn test_command_scope_defaults() {
        let scope = CommandScope::default();
        assert!(scope.allowed_commands.is_empty());
        assert!(scope.denied_commands.contains(&"rm -rf /".to_string()));
        assert!(scope.denied_commands.contains(&"mkfs".to_string()));
        assert!(scope.denied_commands.contains(&"dd".to_string()));
    }

    #[test]
    fn test_command_denied() {
        let scope = CommandScope::default();
        assert!(!scope.is_command_allowed("rm -rf /"));
        assert!(!scope.is_command_allowed("sudo mkfs /dev/sda"));
        assert!(!scope.is_command_allowed("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn test_command_allowed_when_empty_allowlist() {
        let scope = CommandScope::default();
        assert!(scope.is_command_allowed("ls -la"));
        assert!(scope.is_command_allowed("cargo build"));
        assert!(scope.is_command_allowed("git status"));
    }

    #[test]
    fn test_command_with_allowlist() {
        let scope = CommandScope {
            allowed_commands: vec!["cargo".into(), "git".into()],
            denied_commands: vec![],
        };
        assert!(scope.is_command_allowed("cargo build"));
        assert!(scope.is_command_allowed("git push"));
        assert!(!scope.is_command_allowed("rm -rf /tmp"));
    }

    // --- ProviderScope tests ---

    #[test]
    fn test_provider_scope_defaults() {
        let scope = ProviderScope::default();
        assert!(scope.allowed_providers.is_empty());
        assert!(scope.max_cost_per_session_usd.is_none());
        assert!(scope.max_tokens_per_session.is_none());
    }

    #[test]
    fn test_provider_scope_custom() {
        let scope = ProviderScope {
            allowed_providers: vec!["openai".into(), "anthropic".into()],
            max_cost_per_session_usd: Some(10.0),
            max_tokens_per_session: Some(100_000),
        };
        assert_eq!(scope.allowed_providers.len(), 2);
        assert_eq!(scope.max_cost_per_session_usd, Some(10.0));
        assert_eq!(scope.max_tokens_per_session, Some(100_000));
    }

    // --- Serialization tests ---

    #[test]
    fn test_filesystem_scope_serialization() {
        let scope = FilesystemScope::default();
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: FilesystemScope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.allowed_paths, scope.allowed_paths);
        assert_eq!(deserialized.denied_paths, scope.denied_paths);
    }

    #[test]
    fn test_command_scope_serialization() {
        let scope = CommandScope::default();
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: CommandScope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.denied_commands, scope.denied_commands);
    }

    #[test]
    fn test_provider_scope_serialization() {
        let scope = ProviderScope {
            allowed_providers: vec!["openai".into()],
            max_cost_per_session_usd: Some(5.0),
            max_tokens_per_session: Some(50_000),
        };
        let json = serde_json::to_string(&scope).unwrap();
        let deserialized: ProviderScope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.allowed_providers, vec!["openai"]);
        assert_eq!(deserialized.max_cost_per_session_usd, Some(5.0));
        assert_eq!(deserialized.max_tokens_per_session, Some(50_000));
    }
}
