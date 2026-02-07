use serde::{Deserialize, Serialize};

/// Capabilities a plugin can declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    ToolProvider,
    HookHandler,
    ThemeProvider,
    ModelProvider,
    StorageProvider,
    UiExtension,
}

/// Sandbox level for plugin execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxLevel {
    /// No sandbox. Full system access.
    None,
    /// Network-only sandbox. File system restricted.
    NetworkOnly,
    /// Full sandbox. No network, restricted filesystem.
    Full,
}

impl Default for SandboxLevel {
    fn default() -> Self {
        Self::Full
    }
}

/// Negotiation result for plugin capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    pub capability: PluginCapability,
    pub granted: bool,
    pub reason: Option<String>,
}

/// Negotiate which capabilities to grant to a plugin.
pub fn negotiate_capabilities(
    requested: &[PluginCapability],
    allowed: &[PluginCapability],
) -> Vec<CapabilityGrant> {
    requested
        .iter()
        .map(|cap| {
            let granted = allowed.contains(cap);
            CapabilityGrant {
                capability: *cap,
                granted,
                reason: if granted {
                    None
                } else {
                    Some("not in allowed list".into())
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_all_granted() {
        let requested = vec![PluginCapability::ToolProvider, PluginCapability::HookHandler];
        let allowed = vec![
            PluginCapability::ToolProvider,
            PluginCapability::HookHandler,
            PluginCapability::ThemeProvider,
        ];
        let grants = negotiate_capabilities(&requested, &allowed);
        assert_eq!(grants.len(), 2);
        assert!(grants.iter().all(|g| g.granted));
        assert!(grants.iter().all(|g| g.reason.is_none()));
    }

    #[test]
    fn test_negotiate_partial() {
        let requested = vec![PluginCapability::ToolProvider, PluginCapability::UiExtension];
        let allowed = vec![PluginCapability::ToolProvider];
        let grants = negotiate_capabilities(&requested, &allowed);

        assert!(grants[0].granted);
        assert!(grants[0].reason.is_none());

        assert!(!grants[1].granted);
        assert!(grants[1].reason.is_some());
    }

    #[test]
    fn test_negotiate_none_granted() {
        let requested = vec![PluginCapability::ModelProvider];
        let allowed: Vec<PluginCapability> = vec![];
        let grants = negotiate_capabilities(&requested, &allowed);
        assert_eq!(grants.len(), 1);
        assert!(!grants[0].granted);
    }

    #[test]
    fn test_capability_serialization() {
        let json = serde_json::to_string(&PluginCapability::StorageProvider).unwrap();
        assert_eq!(json, "\"storage_provider\"");

        let parsed: PluginCapability = serde_json::from_str("\"ui_extension\"").unwrap();
        assert_eq!(parsed, PluginCapability::UiExtension);
    }

    #[test]
    fn test_sandbox_level_default() {
        assert_eq!(SandboxLevel::default(), SandboxLevel::Full);
    }

    #[test]
    fn test_sandbox_level_ordering() {
        assert!(SandboxLevel::None < SandboxLevel::NetworkOnly);
        assert!(SandboxLevel::NetworkOnly < SandboxLevel::Full);
    }
}
