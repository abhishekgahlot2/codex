use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnboardingStep {
    Welcome,
    SelectProvider,
    ConfigureModel,
    SetPermissionMode,
    EnableFeatures,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingConfig {
    pub providers: Vec<String>,
    pub model_choices: Vec<String>,
    pub permission_modes: Vec<String>,
    pub feature_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub current_step: OnboardingStep,
    pub completed_steps: Vec<OnboardingStep>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub enabled_features: Vec<String>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingProgress {
    pub total_steps: usize,
    pub completed: usize,
    pub current_step_name: String,
    pub is_complete: bool,
}

pub fn default_config() -> OnboardingConfig {
    OnboardingConfig {
        providers: vec![
            "anthropic".to_string(),
            "openai".to_string(),
            "openrouter".to_string(),
        ],
        model_choices: vec![],
        permission_modes: vec![
            "default".to_string(),
            "acceptEdits".to_string(),
            "plan".to_string(),
            "delegate".to_string(),
            "dontAsk".to_string(),
            "bypassPermissions".to_string(),
        ],
        feature_flags: vec![
            "team_orchestration".to_string(),
            "plugins".to_string(),
        ],
    }
}

pub fn step_description(step: &OnboardingStep) -> &str {
    match step {
        OnboardingStep::Welcome => "Welcome to Console — let's get you set up",
        OnboardingStep::SelectProvider => "Choose your AI provider",
        OnboardingStep::ConfigureModel => "Configure which model to use",
        OnboardingStep::SetPermissionMode => "Set the permission mode for tool execution",
        OnboardingStep::EnableFeatures => "Enable optional features like teams and plugins",
        OnboardingStep::Complete => "Onboarding complete — you're ready to go",
    }
}

fn step_order() -> Vec<OnboardingStep> {
    vec![
        OnboardingStep::Welcome,
        OnboardingStep::SelectProvider,
        OnboardingStep::ConfigureModel,
        OnboardingStep::SetPermissionMode,
        OnboardingStep::EnableFeatures,
        OnboardingStep::Complete,
    ]
}

pub fn next_step(state: &OnboardingState) -> Option<OnboardingStep> {
    if state.current_step == OnboardingStep::Complete {
        return None;
    }
    let order = step_order();
    let current_idx = order.iter().position(|s| *s == state.current_step);
    match current_idx {
        Some(idx) if idx + 1 < order.len() => Some(order[idx + 1].clone()),
        _ => None,
    }
}

pub fn advance(state: &mut OnboardingState, step: OnboardingStep) -> Result<(), String> {
    let expected = next_step(state);
    match expected {
        Some(ref expected_step) if *expected_step == step => {
            state.completed_steps.push(state.current_step.clone());
            state.current_step = step;
            Ok(())
        }
        Some(expected_step) => Err(format!(
            "Expected step {:?} but got {:?}",
            expected_step, step
        )),
        None => Err("Onboarding is already complete".to_string()),
    }
}

pub fn progress(state: &OnboardingState) -> OnboardingProgress {
    let total_steps = step_order().len();
    let completed = state.completed_steps.len();
    let current_step_name = format!("{:?}", state.current_step);
    let is_complete = state.current_step == OnboardingStep::Complete;
    OnboardingProgress {
        total_steps,
        completed,
        current_step_name,
        is_complete,
    }
}

pub fn is_onboarding_needed(config_exists: bool) -> bool {
    !config_exists
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_state() -> OnboardingState {
        OnboardingState {
            current_step: OnboardingStep::Welcome,
            completed_steps: vec![],
            provider: None,
            model: None,
            permission_mode: None,
            enabled_features: vec![],
            started_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_default_config_has_providers() {
        let config = default_config();
        assert!(!config.providers.is_empty());
        assert!(config.providers.contains(&"anthropic".to_string()));
        assert!(config.providers.contains(&"openai".to_string()));
        assert!(config.providers.contains(&"openrouter".to_string()));
    }

    #[test]
    fn test_step_descriptions_non_empty() {
        let steps = step_order();
        for step in &steps {
            let desc = step_description(step);
            assert!(!desc.is_empty(), "Description for {:?} should not be empty", step);
        }
    }

    #[test]
    fn test_onboarding_sequence_full() {
        let mut state = new_state();
        assert_eq!(state.current_step, OnboardingStep::Welcome);

        advance(&mut state, OnboardingStep::SelectProvider).unwrap();
        assert_eq!(state.current_step, OnboardingStep::SelectProvider);

        advance(&mut state, OnboardingStep::ConfigureModel).unwrap();
        assert_eq!(state.current_step, OnboardingStep::ConfigureModel);

        advance(&mut state, OnboardingStep::SetPermissionMode).unwrap();
        assert_eq!(state.current_step, OnboardingStep::SetPermissionMode);

        advance(&mut state, OnboardingStep::EnableFeatures).unwrap();
        assert_eq!(state.current_step, OnboardingStep::EnableFeatures);

        advance(&mut state, OnboardingStep::Complete).unwrap();
        assert_eq!(state.current_step, OnboardingStep::Complete);

        assert_eq!(state.completed_steps.len(), 5);
        assert!(next_step(&state).is_none());
    }

    #[test]
    fn test_advance_wrong_step_errors() {
        let mut state = new_state();
        let result = advance(&mut state, OnboardingStep::ConfigureModel);
        assert!(result.is_err());
    }

    #[test]
    fn test_next_step_from_welcome() {
        let state = new_state();
        let next = next_step(&state);
        assert_eq!(next, Some(OnboardingStep::SelectProvider));
    }

    #[test]
    fn test_next_step_after_complete() {
        let state = OnboardingState {
            current_step: OnboardingStep::Complete,
            completed_steps: vec![
                OnboardingStep::Welcome,
                OnboardingStep::SelectProvider,
                OnboardingStep::ConfigureModel,
                OnboardingStep::SetPermissionMode,
                OnboardingStep::EnableFeatures,
            ],
            provider: None,
            model: None,
            permission_mode: None,
            enabled_features: vec![],
            started_at: "2026-01-01T00:00:00Z".to_string(),
        };
        assert!(next_step(&state).is_none());
    }

    #[test]
    fn test_progress_tracking() {
        let mut state = new_state();
        let p = progress(&state);
        assert_eq!(p.total_steps, 6);
        assert_eq!(p.completed, 0);
        assert!(!p.is_complete);

        advance(&mut state, OnboardingStep::SelectProvider).unwrap();
        let p = progress(&state);
        assert_eq!(p.completed, 1);

        advance(&mut state, OnboardingStep::ConfigureModel).unwrap();
        let p = progress(&state);
        assert_eq!(p.completed, 2);

        advance(&mut state, OnboardingStep::SetPermissionMode).unwrap();
        advance(&mut state, OnboardingStep::EnableFeatures).unwrap();
        advance(&mut state, OnboardingStep::Complete).unwrap();
        let p = progress(&state);
        assert_eq!(p.completed, 5);
        assert!(p.is_complete);
    }

    #[test]
    fn test_onboarding_needed() {
        assert!(is_onboarding_needed(false));
        assert!(!is_onboarding_needed(true));
    }
}
