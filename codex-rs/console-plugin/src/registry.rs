use serde::{Deserialize, Serialize};

use crate::capability::{PluginCapability, SandboxLevel};

/// Metadata about a registered plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub capabilities: Vec<PluginCapability>,
    pub sandbox_level: SandboxLevel,
    pub enabled: bool,
}

/// Plugin registry managing installed plugins.
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: Vec<PluginMetadata>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: PluginMetadata) -> Result<(), String> {
        if self.plugins.iter().any(|p| p.name == plugin.name) {
            return Err(format!("plugin '{}' already registered", plugin.name));
        }
        self.plugins.push(plugin);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&PluginMetadata> {
        self.plugins.iter().find(|p| p.name == name)
    }

    pub fn list_enabled(&self) -> Vec<&PluginMetadata> {
        self.plugins.iter().filter(|p| p.enabled).collect()
    }

    pub fn list_by_capability(&self, cap: PluginCapability) -> Vec<&PluginMetadata> {
        self.plugins
            .iter()
            .filter(|p| p.capabilities.contains(&cap))
            .collect()
    }

    pub fn all(&self) -> &[PluginMetadata] {
        &self.plugins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_plugin(name: &str, caps: Vec<PluginCapability>, enabled: bool) -> PluginMetadata {
        PluginMetadata {
            name: name.into(),
            version: "1.0.0".into(),
            description: format!("{name} plugin"),
            author: None,
            capabilities: caps,
            sandbox_level: SandboxLevel::default(),
            enabled,
        }
    }

    #[test]
    fn test_register_and_lookup() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("alpha", vec![PluginCapability::ToolProvider], true))
            .unwrap();

        let p = reg.get("alpha").unwrap();
        assert_eq!(p.name, "alpha");
        assert!(reg.get("beta").is_none());
    }

    #[test]
    fn test_duplicate_rejection() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("dup", vec![], true)).unwrap();
        let err = reg.register(make_plugin("dup", vec![], true)).unwrap_err();
        assert!(err.contains("already registered"));
    }

    #[test]
    fn test_list_by_capability() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin(
            "a",
            vec![PluginCapability::ToolProvider],
            true,
        ))
        .unwrap();
        reg.register(make_plugin(
            "b",
            vec![PluginCapability::ThemeProvider],
            true,
        ))
        .unwrap();
        reg.register(make_plugin(
            "c",
            vec![PluginCapability::ToolProvider, PluginCapability::UiExtension],
            true,
        ))
        .unwrap();

        let tool_providers = reg.list_by_capability(PluginCapability::ToolProvider);
        assert_eq!(tool_providers.len(), 2);
    }

    #[test]
    fn test_list_enabled() {
        let mut reg = PluginRegistry::new();
        reg.register(make_plugin("on", vec![], true)).unwrap();
        reg.register(make_plugin("off", vec![], false)).unwrap();

        let enabled = reg.list_enabled();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "on");
    }

    #[test]
    fn test_plugin_metadata_serialization() {
        let plugin = make_plugin("test", vec![PluginCapability::ModelProvider], true);
        let json = serde_json::to_string(&plugin).unwrap();
        let parsed: PluginMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.capabilities, vec![PluginCapability::ModelProvider]);
    }
}
