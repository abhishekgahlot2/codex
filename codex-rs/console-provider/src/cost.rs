use crate::registry::ModelRegistry;

/// Token usage counts for a single request.
#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
}

/// Cost breakdown for a single request.
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    /// Cost for non-cached input tokens.
    pub input_cost: f64,
    /// Cost for output tokens.
    pub output_cost: f64,
    /// Savings from cached input tokens (vs full-price input).
    pub cache_savings: f64,
    /// Total cost (input + output, with cache discount applied).
    pub total_cost: f64,
}

/// Calculates token costs based on a model registry.
pub struct TokenCostCalculator {
    registry: ModelRegistry,
}

impl TokenCostCalculator {
    /// Create a new calculator backed by the given registry.
    pub fn new(registry: &ModelRegistry) -> Self {
        Self {
            registry: registry.clone(),
        }
    }

    /// Calculate the cost breakdown for a given model and usage.
    /// Returns `None` if the model is not found in the registry.
    pub fn calculate(&self, model_id: &str, usage: &TokenUsage) -> Option<CostBreakdown> {
        let info = self.registry.get(model_id)?;

        let non_cached_input = usage.input_tokens.saturating_sub(usage.cached_input_tokens);
        let input_cost = (non_cached_input as f64) * info.input_cost_per_mtok / 1_000_000.0;
        let cached_cost =
            (usage.cached_input_tokens as f64) * info.cached_input_cost_per_mtok / 1_000_000.0;
        let output_cost = (usage.output_tokens as f64) * info.output_cost_per_mtok / 1_000_000.0;

        // Savings: what the cached tokens would have cost at full price minus what
        // they actually cost at the cached rate.
        let full_price_cached =
            (usage.cached_input_tokens as f64) * info.input_cost_per_mtok / 1_000_000.0;
        let cache_savings = full_price_cached - cached_cost;

        let total_cost = input_cost + cached_cost + output_cost;

        Some(CostBreakdown {
            input_cost,
            output_cost,
            cache_savings,
            total_cost,
        })
    }

    /// Format a cost value as a USD string, e.g. `"$0.0042"`.
    pub fn format_cost(cost: f64) -> String {
        if cost < 0.01 {
            format!("${cost:.4}")
        } else {
            format!("${cost:.2}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::default_registry;

    #[test]
    fn calculate_cost_for_known_model() {
        let reg = default_registry();
        let calc = TokenCostCalculator::new(&reg);

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cached_input_tokens: 0,
        };

        let breakdown = calc.calculate("claude-opus-4-6", &usage);
        assert!(breakdown.is_some());
        let b = breakdown.unwrap();
        // $15 per Mtok input, $75 per Mtok output
        assert!((b.input_cost - 15.0).abs() < 0.001);
        assert!((b.output_cost - 75.0).abs() < 0.001);
        assert!((b.total_cost - 90.0).abs() < 0.001);
        assert!((b.cache_savings - 0.0).abs() < 0.001);
    }

    #[test]
    fn calculate_cost_with_cache() {
        let reg = default_registry();
        let calc = TokenCostCalculator::new(&reg);

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cached_input_tokens: 500_000,
        };

        let breakdown = calc.calculate("claude-opus-4-6", &usage).unwrap();
        // 500k non-cached at $15/Mtok = $7.50
        // 500k cached at $1.50/Mtok = $0.75
        // total = $8.25
        assert!((breakdown.input_cost - 7.5).abs() < 0.001);
        assert!((breakdown.total_cost - 8.25).abs() < 0.001);
        // Savings: 500k * ($15 - $1.50) / 1M = $6.75
        assert!((breakdown.cache_savings - 6.75).abs() < 0.001);
    }

    #[test]
    fn calculate_unknown_model_returns_none() {
        let reg = default_registry();
        let calc = TokenCostCalculator::new(&reg);

        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 100,
            cached_input_tokens: 0,
        };

        assert!(calc.calculate("nonexistent-model", &usage).is_none());
    }

    #[test]
    fn format_cost_display() {
        assert_eq!(TokenCostCalculator::format_cost(0.0042), "$0.0042");
        assert_eq!(TokenCostCalculator::format_cost(1.50), "$1.50");
        assert_eq!(TokenCostCalculator::format_cost(0.0), "$0.0000");
    }
}
