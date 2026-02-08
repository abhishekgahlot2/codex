use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleCliArgs {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub enable_teams: bool,
    pub enable_plugins: bool,
    pub onboarding: bool,
    pub export_session: Option<String>,
    pub import_session: Option<String>,
    pub verbose: bool,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgsValidationError {
    InvalidProvider(String),
    InvalidMode(String),
    ConflictingFlags(String),
}

impl std::fmt::Display for ArgsValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgsValidationError::InvalidProvider(p) => write!(f, "Invalid provider: {p}"),
            ArgsValidationError::InvalidMode(m) => write!(f, "Invalid mode: {m}"),
            ArgsValidationError::ConflictingFlags(msg) => write!(f, "Conflicting flags: {msg}"),
        }
    }
}

pub fn default_args() -> ConsoleCliArgs {
    ConsoleCliArgs {
        provider: None,
        model: None,
        permission_mode: None,
        enable_teams: true,
        enable_plugins: false,
        onboarding: false,
        export_session: None,
        import_session: None,
        verbose: false,
        config_path: None,
    }
}

pub fn known_providers() -> Vec<&'static str> {
    vec!["anthropic", "openai", "openrouter", "ollama"]
}

pub fn known_modes() -> Vec<&'static str> {
    vec![
        "default",
        "acceptEdits",
        "plan",
        "delegate",
        "dontAsk",
        "bypassPermissions",
    ]
}

pub fn validate(args: &ConsoleCliArgs) -> Result<(), ArgsValidationError> {
    if let Some(ref provider) = args.provider {
        if !known_providers().contains(&provider.as_str()) {
            return Err(ArgsValidationError::InvalidProvider(provider.clone()));
        }
    }
    if let Some(ref mode) = args.permission_mode {
        if !known_modes().contains(&mode.as_str()) {
            return Err(ArgsValidationError::InvalidMode(mode.clone()));
        }
    }
    if args.export_session.is_some() && args.import_session.is_some() {
        return Err(ArgsValidationError::ConflictingFlags(
            "export_session and import_session cannot both be set".to_string(),
        ));
    }
    Ok(())
}

pub fn merge_with_env(args: &mut ConsoleCliArgs) {
    if args.provider.is_none() {
        if let Ok(val) = std::env::var("CONSOLE_PROVIDER") {
            args.provider = Some(val);
        }
    }
    if args.model.is_none() {
        if let Ok(val) = std::env::var("CONSOLE_MODEL") {
            args.model = Some(val);
        }
    }
    if args.permission_mode.is_none() {
        if let Ok(val) = std::env::var("CONSOLE_MODE") {
            args.permission_mode = Some(val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_args() {
        let args = default_args();
        assert!(args.provider.is_none());
        assert!(args.model.is_none());
        assert!(args.permission_mode.is_none());
        assert!(args.enable_teams);
        assert!(!args.enable_plugins);
        assert!(!args.onboarding);
        assert!(args.export_session.is_none());
        assert!(args.import_session.is_none());
        assert!(!args.verbose);
        assert!(args.config_path.is_none());
    }

    #[test]
    fn test_validate_valid_args() {
        let args = ConsoleCliArgs {
            provider: Some("anthropic".to_string()),
            permission_mode: Some("plan".to_string()),
            ..default_args()
        };
        assert!(validate(&args).is_ok());
    }

    #[test]
    fn test_validate_invalid_provider() {
        let args = ConsoleCliArgs {
            provider: Some("unknown_provider".to_string()),
            ..default_args()
        };
        assert_eq!(
            validate(&args),
            Err(ArgsValidationError::InvalidProvider(
                "unknown_provider".to_string()
            ))
        );
    }

    #[test]
    fn test_validate_invalid_mode() {
        let args = ConsoleCliArgs {
            permission_mode: Some("yolo".to_string()),
            ..default_args()
        };
        assert_eq!(
            validate(&args),
            Err(ArgsValidationError::InvalidMode("yolo".to_string()))
        );
    }

    #[test]
    fn test_validate_conflicting_export_import() {
        let args = ConsoleCliArgs {
            export_session: Some("out.json".to_string()),
            import_session: Some("in.json".to_string()),
            ..default_args()
        };
        assert!(matches!(
            validate(&args),
            Err(ArgsValidationError::ConflictingFlags(_))
        ));
    }

    #[test]
    fn test_known_providers_complete() {
        let providers = known_providers();
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"openrouter"));
        assert!(providers.contains(&"ollama"));
        assert_eq!(providers.len(), 4);
    }
}
