/// Metadata for a known model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model identifier, e.g. `"claude-opus-4-6"`.
    pub id: &'static str,
    /// Provider name, e.g. `"anthropic"`.
    pub provider: &'static str,
    /// Human-friendly display name, e.g. `"Claude Opus 4.6"`.
    pub display_name: &'static str,
    /// Maximum context window in tokens.
    pub context_window: u64,
    /// Maximum output tokens per request.
    pub max_output_tokens: u64,
    /// Cost in USD per million input tokens.
    pub input_cost_per_mtok: f64,
    /// Cost in USD per million output tokens.
    pub output_cost_per_mtok: f64,
    /// Cost in USD per million cached input tokens.
    pub cached_input_cost_per_mtok: f64,
    /// Whether this model supports tool use.
    pub supports_tools: bool,
    /// Whether this model supports vision (image inputs).
    pub supports_vision: bool,
    /// Whether this model supports streaming responses.
    pub supports_streaming: bool,
}

/// A static registry of known models and their metadata.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    models: Vec<ModelInfo>,
}

impl ModelRegistry {
    /// Create a new registry from a list of model info entries.
    pub fn new(models: Vec<ModelInfo>) -> Self {
        Self { models }
    }

    /// Look up a model by its identifier.
    pub fn get(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == model_id)
    }

    /// Return all registered models.
    pub fn all(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Number of registered models.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// Returns a registry pre-populated with well-known models.
pub fn default_registry() -> ModelRegistry {
    ModelRegistry::new(vec![
        ModelInfo {
            id: "claude-opus-4-6",
            provider: "anthropic",
            display_name: "Claude Opus 4.6",
            context_window: 200_000,
            max_output_tokens: 32_000,
            input_cost_per_mtok: 15.0,
            output_cost_per_mtok: 75.0,
            cached_input_cost_per_mtok: 1.5,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "claude-sonnet-4-5-20250929",
            provider: "anthropic",
            display_name: "Claude Sonnet 4.5",
            context_window: 200_000,
            max_output_tokens: 16_000,
            input_cost_per_mtok: 3.0,
            output_cost_per_mtok: 15.0,
            cached_input_cost_per_mtok: 0.3,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "claude-haiku-4-5-20251001",
            provider: "anthropic",
            display_name: "Claude Haiku 4.5",
            context_window: 200_000,
            max_output_tokens: 8_192,
            input_cost_per_mtok: 0.80,
            output_cost_per_mtok: 4.0,
            cached_input_cost_per_mtok: 0.08,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "gpt-4o",
            provider: "openai",
            display_name: "GPT-4o",
            context_window: 128_000,
            max_output_tokens: 16_384,
            input_cost_per_mtok: 2.50,
            output_cost_per_mtok: 10.0,
            cached_input_cost_per_mtok: 1.25,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "gpt-4o-mini",
            provider: "openai",
            display_name: "GPT-4o mini",
            context_window: 128_000,
            max_output_tokens: 16_384,
            input_cost_per_mtok: 0.15,
            output_cost_per_mtok: 0.60,
            cached_input_cost_per_mtok: 0.075,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "o3",
            provider: "openai",
            display_name: "o3",
            context_window: 200_000,
            max_output_tokens: 100_000,
            input_cost_per_mtok: 10.0,
            output_cost_per_mtok: 40.0,
            cached_input_cost_per_mtok: 2.50,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
        ModelInfo {
            id: "o4-mini",
            provider: "openai",
            display_name: "o4-mini",
            context_window: 200_000,
            max_output_tokens: 100_000,
            input_cost_per_mtok: 1.10,
            output_cost_per_mtok: 4.40,
            cached_input_cost_per_mtok: 0.275,
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_expected_models() {
        let reg = default_registry();
        assert_eq!(reg.len(), 7);
        assert!(!reg.is_empty());

        let ids: Vec<&str> = reg.all().iter().map(|m| m.id).collect();
        assert!(ids.contains(&"claude-opus-4-6"));
        assert!(ids.contains(&"claude-sonnet-4-5-20250929"));
        assert!(ids.contains(&"claude-haiku-4-5-20251001"));
        assert!(ids.contains(&"gpt-4o"));
        assert!(ids.contains(&"gpt-4o-mini"));
        assert!(ids.contains(&"o3"));
        assert!(ids.contains(&"o4-mini"));
    }

    #[test]
    fn lookup_by_id_works() {
        let reg = default_registry();

        let opus = reg.get("claude-opus-4-6");
        assert!(opus.is_some());
        let opus = opus.unwrap();
        assert_eq!(opus.provider, "anthropic");
        assert_eq!(opus.display_name, "Claude Opus 4.6");
        assert_eq!(opus.context_window, 200_000);
        assert_eq!(opus.input_cost_per_mtok, 15.0);
        assert_eq!(opus.output_cost_per_mtok, 75.0);
    }

    #[test]
    fn lookup_unknown_model_returns_none() {
        let reg = default_registry();
        assert!(reg.get("nonexistent-model").is_none());
    }
}
