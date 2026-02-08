use serde::Deserialize;
use serde::Serialize;

use crate::session::DurableSession;
use crate::session::SessionError;

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    Markdown,
}

/// Export a session to a string in the given format.
pub fn export_session(
    session: &DurableSession,
    format: ExportFormat,
) -> Result<String, SessionError> {
    match format {
        ExportFormat::Json => serde_json::to_string_pretty(session)
            .map_err(|e| SessionError::Serialization(e.to_string())),
        ExportFormat::Markdown => {
            let mut md = String::new();
            md.push_str(&format!("# Session: {}\n\n", session.session_id));
            if let Some(ref model) = session.model {
                md.push_str(&format!("**Model**: {model}\n"));
            }
            md.push_str(&format!("**Messages**: {}\n", session.messages.len()));
            md.push_str(&format!("**Tokens**: {}\n", session.total_tokens));
            md.push_str(&format!("**Cost**: ${:.4}\n\n", session.total_cost_usd));
            md.push_str("---\n\n");
            for msg in &session.messages {
                md.push_str(&format!("### {} ({})\n\n", msg.role, msg.timestamp));
                md.push_str(&msg.content);
                md.push_str("\n\n");
            }
            Ok(md)
        }
    }
}

/// Import a session from a JSON string.
pub fn import_session(json: &str) -> Result<DurableSession, SessionError> {
    serde_json::from_str(json).map_err(|e| SessionError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> DurableSession {
        let mut session = DurableSession::new("export-test");
        session.model = Some("gpt-4".to_string());
        session.total_tokens = 1234;
        session.total_cost_usd = 0.0567;
        session.add_message("user", "Hello world");
        session.add_message("assistant", "Hi there!");
        session
    }

    #[test]
    fn test_export_json_roundtrip() {
        let session = sample_session();
        let json = export_session(&session, ExportFormat::Json).unwrap();
        let imported = import_session(&json).unwrap();
        assert_eq!(imported.session_id, "export-test");
        assert_eq!(imported.message_count(), 2);
        assert_eq!(imported.model, Some("gpt-4".to_string()));
        assert_eq!(imported.total_tokens, 1234);
    }

    #[test]
    fn test_export_markdown_contains_key_info() {
        let session = sample_session();
        let md = export_session(&session, ExportFormat::Markdown).unwrap();
        assert!(md.contains("# Session: export-test"));
        assert!(md.contains("**Model**: gpt-4"));
        assert!(md.contains("**Messages**: 2"));
        assert!(md.contains("**Tokens**: 1234"));
        assert!(md.contains("$0.0567"));
        assert!(md.contains("### user"));
        assert!(md.contains("### assistant"));
        assert!(md.contains("Hello world"));
        assert!(md.contains("Hi there!"));
    }

    #[test]
    fn test_import_from_json() {
        let session = sample_session();
        let json = serde_json::to_string(&session).unwrap();
        let imported = import_session(&json).unwrap();
        assert_eq!(imported.session_id, session.session_id);
        assert_eq!(imported.message_count(), session.message_count());
    }

    #[test]
    fn test_import_invalid_json_returns_error() {
        let result = import_session("this is not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_export_format_serialization() {
        let json_fmt = ExportFormat::Json;
        let serialized = serde_json::to_string(&json_fmt).unwrap();
        assert_eq!(serialized, "\"json\"");

        let md_fmt: ExportFormat = serde_json::from_str("\"markdown\"").unwrap();
        assert_eq!(md_fmt, ExportFormat::Markdown);
    }
}
