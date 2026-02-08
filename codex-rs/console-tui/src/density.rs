use serde::Deserialize;
use serde::Serialize;

/// Conversation display density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationDensity {
    /// Minimal spacing, no separators.
    Compact,
    /// Standard spacing with subtle separators.
    Normal,
    /// Extra spacing, prominent separators.
    Relaxed,
}

impl Default for ConversationDensity {
    fn default() -> Self {
        Self::Normal
    }
}

/// Spacing configuration derived from density setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityConfig {
    /// Density level.
    pub density: ConversationDensity,
    /// Lines of padding between messages.
    pub message_gap: u16,
    /// Lines of padding inside a message bubble.
    pub message_padding: u16,
    /// Whether to show separator lines between messages.
    pub show_separators: bool,
    /// Whether to show timestamps on each message.
    pub show_timestamps: bool,
    /// Whether to collapse consecutive tool results.
    pub collapse_tool_results: bool,
}

/// Get the density config for a given density level.
pub fn density_config(density: ConversationDensity) -> DensityConfig {
    match density {
        ConversationDensity::Compact => DensityConfig {
            density,
            message_gap: 0,
            message_padding: 0,
            show_separators: false,
            show_timestamps: false,
            collapse_tool_results: true,
        },
        ConversationDensity::Normal => DensityConfig {
            density,
            message_gap: 1,
            message_padding: 0,
            show_separators: true,
            show_timestamps: false,
            collapse_tool_results: false,
        },
        ConversationDensity::Relaxed => DensityConfig {
            density,
            message_gap: 2,
            message_padding: 1,
            show_separators: true,
            show_timestamps: true,
            collapse_tool_results: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_density_is_normal() {
        assert_eq!(ConversationDensity::default(), ConversationDensity::Normal);
    }

    #[test]
    fn compact_config() {
        let cfg = density_config(ConversationDensity::Compact);
        assert_eq!(cfg.density, ConversationDensity::Compact);
        assert_eq!(cfg.message_gap, 0);
        assert_eq!(cfg.message_padding, 0);
        assert!(!cfg.show_separators);
        assert!(!cfg.show_timestamps);
        assert!(cfg.collapse_tool_results);
    }

    #[test]
    fn normal_config() {
        let cfg = density_config(ConversationDensity::Normal);
        assert_eq!(cfg.density, ConversationDensity::Normal);
        assert_eq!(cfg.message_gap, 1);
        assert_eq!(cfg.message_padding, 0);
        assert!(cfg.show_separators);
        assert!(!cfg.show_timestamps);
        assert!(!cfg.collapse_tool_results);
    }

    #[test]
    fn relaxed_config() {
        let cfg = density_config(ConversationDensity::Relaxed);
        assert_eq!(cfg.density, ConversationDensity::Relaxed);
        assert_eq!(cfg.message_gap, 2);
        assert_eq!(cfg.message_padding, 1);
        assert!(cfg.show_separators);
        assert!(cfg.show_timestamps);
        assert!(!cfg.collapse_tool_results);
    }

    #[test]
    fn serialization_roundtrip() {
        for density in [
            ConversationDensity::Compact,
            ConversationDensity::Normal,
            ConversationDensity::Relaxed,
        ] {
            let cfg = density_config(density);
            let json = serde_json::to_string(&cfg).unwrap();
            let deserialized: DensityConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized.density, density);
            assert_eq!(deserialized.message_gap, cfg.message_gap);
            assert_eq!(deserialized.message_padding, cfg.message_padding);
            assert_eq!(deserialized.show_separators, cfg.show_separators);
            assert_eq!(deserialized.show_timestamps, cfg.show_timestamps);
            assert_eq!(
                deserialized.collapse_tool_results,
                cfg.collapse_tool_results
            );
        }
    }

    #[test]
    fn density_enum_serialization() {
        let json = serde_json::to_string(&ConversationDensity::Compact).unwrap();
        assert_eq!(json, "\"compact\"");
        let json = serde_json::to_string(&ConversationDensity::Normal).unwrap();
        assert_eq!(json, "\"normal\"");
        let json = serde_json::to_string(&ConversationDensity::Relaxed).unwrap();
        assert_eq!(json, "\"relaxed\"");
    }
}
