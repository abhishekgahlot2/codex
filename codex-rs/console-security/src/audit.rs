use serde::{Deserialize, Serialize};

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub action: String,
    pub actor: String,
    pub decision: String,
    pub details: Option<String>,
    pub timestamp: String,
    pub redacted: bool,
}

/// Audit log buffer.
#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    max_entries: usize,
}

impl AuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn record(
        &mut self,
        action: &str,
        actor: &str,
        decision: &str,
        details: Option<&str>,
    ) {
        let id = format!("audit-{}", self.entries.len() + 1);
        self.entries.push(AuditEntry {
            id,
            action: action.into(),
            actor: actor.into(),
            decision: decision.into(),
            details: details.map(|s| s.into()),
            timestamp: String::new(), // Caller populates
            redacted: false,
        });
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Patterns for content that should be redacted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub patterns: Vec<String>,
    pub replacement: String,
}

impl RedactionPolicy {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                r"(?i)(api[_-]?key|secret|token|password)\s*[=:]\s*\S+".into(),
                r"sk-[a-zA-Z0-9]{20,}".into(),
                r"Bearer\s+[a-zA-Z0-9._-]+".into(),
            ],
            replacement: "[REDACTED]".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_audit_log_is_empty() {
        let log = AuditLog::new(100);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert!(log.entries().is_empty());
    }

    #[test]
    fn test_record_entry() {
        let mut log = AuditLog::new(100);
        log.record("file:write", "user", "allow", Some("wrote foo.rs"));
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());

        let entry = &log.entries()[0];
        assert_eq!(entry.id, "audit-1");
        assert_eq!(entry.action, "file:write");
        assert_eq!(entry.actor, "user");
        assert_eq!(entry.decision, "allow");
        assert_eq!(entry.details.as_deref(), Some("wrote foo.rs"));
        assert!(!entry.redacted);
    }

    #[test]
    fn test_record_multiple_entries() {
        let mut log = AuditLog::new(100);
        log.record("file:write", "user", "allow", None);
        log.record("tool:exec", "agent", "deny", Some("dangerous"));
        log.record("file:read", "user", "allow", None);

        assert_eq!(log.len(), 3);
        assert_eq!(log.entries()[0].id, "audit-1");
        assert_eq!(log.entries()[1].id, "audit-2");
        assert_eq!(log.entries()[2].id, "audit-3");
    }

    #[test]
    fn test_max_entries_cap() {
        let mut log = AuditLog::new(3);
        log.record("action-1", "user", "allow", None);
        log.record("action-2", "user", "allow", None);
        log.record("action-3", "user", "allow", None);
        assert_eq!(log.len(), 3);

        // Adding a fourth should evict the first
        log.record("action-4", "user", "allow", None);
        assert_eq!(log.len(), 3);
        assert_eq!(log.entries()[0].action, "action-2");
        assert_eq!(log.entries()[2].action, "action-4");
    }

    #[test]
    fn test_record_without_details() {
        let mut log = AuditLog::new(100);
        log.record("file:read", "agent", "allow", None);
        assert!(log.entries()[0].details.is_none());
    }

    #[test]
    fn test_default_audit_log() {
        let log = AuditLog::default();
        assert!(log.is_empty());
        assert_eq!(log.max_entries, 0);
    }

    #[test]
    fn test_redaction_policy_defaults() {
        let policy = RedactionPolicy::new();
        assert_eq!(policy.replacement, "[REDACTED]");
        assert_eq!(policy.patterns.len(), 3);
    }

    #[test]
    fn test_redaction_policy_serialization() {
        let policy = RedactionPolicy::new();
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: RedactionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.replacement, "[REDACTED]");
        assert_eq!(deserialized.patterns.len(), 3);
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry {
            id: "audit-1".into(),
            action: "file:write".into(),
            actor: "user".into(),
            decision: "allow".into(),
            details: Some("wrote foo.rs".into()),
            timestamp: "2025-01-01T00:00:00Z".into(),
            redacted: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "audit-1");
        assert_eq!(deserialized.action, "file:write");
        assert_eq!(deserialized.timestamp, "2025-01-01T00:00:00Z");
    }
}
