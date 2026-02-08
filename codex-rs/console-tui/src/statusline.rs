use serde::Deserialize;
use serde::Serialize;

/// Data model for the statusline display.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatuslineData {
    /// Current model name (e.g., "Claude Opus 4.6").
    pub model: Option<String>,
    /// Current execution mode (e.g., "build", "plan", "review").
    pub mode: Option<String>,
    /// Session cost so far (formatted, e.g., "$0.42").
    pub cost: Option<String>,
    /// Total tokens used this session.
    pub total_tokens: Option<u64>,
    /// Team name if a team is active.
    pub team: Option<String>,
    /// Number of active teammates.
    pub active_agents: Option<u32>,
    /// Number of pending tasks.
    pub pending_tasks: Option<u32>,
    /// Provider name (e.g., "Anthropic", "OpenAI").
    pub provider: Option<String>,
    /// Custom segments added by plugins/MCP.
    pub custom_segments: Vec<StatuslineSegment>,
}

/// A single segment in the statusline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineSegment {
    /// Segment label (short, e.g., "Cost").
    pub label: String,
    /// Segment value (e.g., "$0.42").
    pub value: String,
    /// Optional icon/emoji prefix.
    pub icon: Option<String>,
    /// Priority (lower = rendered first, higher = dropped when space constrained).
    pub priority: u32,
}

impl StatuslineData {
    /// Build a flat list of all segments for rendering, sorted by priority.
    pub fn to_segments(&self) -> Vec<StatuslineSegment> {
        let mut segments = Vec::new();

        if let Some(ref model) = self.model {
            segments.push(StatuslineSegment {
                label: "Model".into(),
                value: model.clone(),
                icon: None,
                priority: 0,
            });
        }
        if let Some(ref mode) = self.mode {
            segments.push(StatuslineSegment {
                label: "Mode".into(),
                value: mode.clone(),
                icon: None,
                priority: 1,
            });
        }
        if let Some(ref provider) = self.provider {
            segments.push(StatuslineSegment {
                label: "Provider".into(),
                value: provider.clone(),
                icon: None,
                priority: 2,
            });
        }
        if let Some(ref cost) = self.cost {
            segments.push(StatuslineSegment {
                label: "Cost".into(),
                value: cost.clone(),
                icon: None,
                priority: 3,
            });
        }
        if let Some(tokens) = self.total_tokens {
            segments.push(StatuslineSegment {
                label: "Tokens".into(),
                value: format!("{tokens}"),
                icon: None,
                priority: 4,
            });
        }
        if let Some(ref team) = self.team {
            segments.push(StatuslineSegment {
                label: "Team".into(),
                value: team.clone(),
                icon: None,
                priority: 5,
            });
        }

        // Add custom segments
        segments.extend(self.custom_segments.clone());

        segments.sort_by_key(|s| s.priority);
        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let data = StatuslineData::default();
        assert!(data.model.is_none());
        assert!(data.mode.is_none());
        assert!(data.cost.is_none());
        assert!(data.total_tokens.is_none());
        assert!(data.team.is_none());
        assert!(data.active_agents.is_none());
        assert!(data.pending_tasks.is_none());
        assert!(data.provider.is_none());
        assert!(data.custom_segments.is_empty());
        assert!(data.to_segments().is_empty());
    }

    #[test]
    fn to_segments_ordering() {
        let data = StatuslineData {
            model: Some("Claude Opus 4.6".into()),
            mode: Some("build".into()),
            cost: Some("$0.42".into()),
            ..Default::default()
        };
        let segments = data.to_segments();
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].label, "Model");
        assert_eq!(segments[1].label, "Mode");
        assert_eq!(segments[2].label, "Cost");
    }

    #[test]
    fn all_fields_populated() {
        let data = StatuslineData {
            model: Some("Claude Opus 4.6".into()),
            mode: Some("build".into()),
            cost: Some("$1.23".into()),
            total_tokens: Some(50000),
            team: Some("my-team".into()),
            active_agents: Some(3),
            pending_tasks: Some(5),
            provider: Some("Anthropic".into()),
            custom_segments: vec![],
        };
        let segments = data.to_segments();
        assert_eq!(segments.len(), 6);
        // Verify priority ordering: Model(0), Mode(1), Provider(2), Cost(3), Tokens(4), Team(5)
        assert_eq!(segments[0].label, "Model");
        assert_eq!(segments[1].label, "Mode");
        assert_eq!(segments[2].label, "Provider");
        assert_eq!(segments[3].label, "Cost");
        assert_eq!(segments[4].label, "Tokens");
        assert_eq!(segments[4].value, "50000");
        assert_eq!(segments[5].label, "Team");
    }

    #[test]
    fn custom_segments_included() {
        let data = StatuslineData {
            model: Some("GPT-4".into()),
            custom_segments: vec![StatuslineSegment {
                label: "Plugin".into(),
                value: "active".into(),
                icon: Some("P".into()),
                priority: 1, // Should sort between Model(0) and others
            }],
            ..Default::default()
        };
        let segments = data.to_segments();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].label, "Model"); // priority 0
        assert_eq!(segments[1].label, "Plugin"); // priority 1
    }

    #[test]
    fn serialization_roundtrip() {
        let data = StatuslineData {
            model: Some("Claude Opus 4.6".into()),
            mode: Some("plan".into()),
            cost: Some("$0.10".into()),
            total_tokens: Some(1234),
            team: Some("test-team".into()),
            active_agents: Some(2),
            pending_tasks: Some(3),
            provider: Some("Anthropic".into()),
            custom_segments: vec![StatuslineSegment {
                label: "Custom".into(),
                value: "val".into(),
                icon: None,
                priority: 10,
            }],
        };
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: StatuslineData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, data.model);
        assert_eq!(deserialized.mode, data.mode);
        assert_eq!(deserialized.cost, data.cost);
        assert_eq!(deserialized.total_tokens, data.total_tokens);
        assert_eq!(deserialized.team, data.team);
        assert_eq!(deserialized.custom_segments.len(), 1);
    }
}
