use serde::{Deserialize, Serialize};

/// A persisted conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedMessage {
    pub id: String,
    pub role: String, // "user", "assistant", "tool", "system"
    pub content: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub timestamp: String,
}

/// A persisted team member snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTeammate {
    pub id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub model: Option<String>,
}

/// A persisted task snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: Option<String>,
    pub depends_on: Vec<String>,
}

/// Full session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableSession {
    pub session_id: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub mode: Option<String>,
    pub messages: Vec<PersistedMessage>,
    pub teammates: Vec<PersistedTeammate>,
    pub tasks: Vec<PersistedTask>,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl DurableSession {
    pub fn new(session_id: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            session_id: session_id.to_string(),
            model: None,
            provider: None,
            mode: None,
            messages: Vec::new(),
            teammates: Vec::new(),
            tasks: Vec::new(),
            total_tokens: 0,
            total_cost_usd: 0.0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        let id = format!("msg-{}", self.messages.len() + 1);
        self.messages.push(PersistedMessage {
            id,
            role: role.into(),
            content: content.into(),
            tool_name: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

/// Trait for session storage backends.
pub trait SessionStore {
    fn save(&self, session: &DurableSession) -> Result<(), SessionError>;
    fn load(&self, session_id: &str) -> Result<DurableSession, SessionError>;
    fn list_sessions(&self) -> Result<Vec<String>, SessionError>;
    fn delete(&self, session_id: &str) -> Result<(), SessionError>;
}

/// JSON file-based session store.
pub struct JsonFileStore {
    base_dir: std::path::PathBuf,
}

impl JsonFileStore {
    pub fn new(base_dir: std::path::PathBuf) -> Self {
        Self { base_dir }
    }

    fn session_path(&self, session_id: &str) -> std::path::PathBuf {
        self.base_dir.join(format!("{session_id}.json"))
    }
}

impl SessionStore for JsonFileStore {
    fn save(&self, session: &DurableSession) -> Result<(), SessionError> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| SessionError::Io(e.to_string()))?;
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| SessionError::Serialization(e.to_string()))?;
        std::fs::write(self.session_path(&session.session_id), json)
            .map_err(|e| SessionError::Io(e.to_string()))
    }

    fn load(&self, session_id: &str) -> Result<DurableSession, SessionError> {
        let path = self.session_path(session_id);
        let data =
            std::fs::read_to_string(&path).map_err(|e| SessionError::Io(e.to_string()))?;
        serde_json::from_str(&data).map_err(|e| SessionError::Serialization(e.to_string()))
    }

    fn list_sessions(&self) -> Result<Vec<String>, SessionError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut ids = Vec::new();
        for entry in
            std::fs::read_dir(&self.base_dir).map_err(|e| SessionError::Io(e.to_string()))?
        {
            let entry = entry.map_err(|e| SessionError::Io(e.to_string()))?;
            if let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("json") {
                    ids.push(name.to_string());
                }
            }
        }
        Ok(ids)
    }

    fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        let path = self.session_path(session_id);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| SessionError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("io error: {0}")]
    Io(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("session not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "console-persist-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn test_session_creation() {
        let session = DurableSession::new("test-1");
        assert_eq!(session.session_id, "test-1");
        assert_eq!(session.message_count(), 0);
        assert!(session.model.is_none());
        assert_eq!(session.total_tokens, 0);
    }

    #[test]
    fn test_add_messages() {
        let mut session = DurableSession::new("test-2");
        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi there!");
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(session.messages[1].role, "assistant");
        assert_eq!(session.messages[1].id, "msg-2");
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = temp_dir();
        let store = JsonFileStore::new(dir.clone());
        let mut session = DurableSession::new("roundtrip-1");
        session.add_message("user", "Test message");
        session.model = Some("gpt-4".to_string());
        session.total_tokens = 100;

        store.save(&session).ok();
        let loaded = store.load("roundtrip-1").ok();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.session_id, "roundtrip-1");
        assert_eq!(loaded.message_count(), 1);
        assert_eq!(loaded.model, Some("gpt-4".to_string()));
        assert_eq!(loaded.total_tokens, 100);

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_sessions() {
        let dir = temp_dir();
        let store = JsonFileStore::new(dir.clone());

        let s1 = DurableSession::new("list-a");
        let s2 = DurableSession::new("list-b");
        store.save(&s1).ok();
        store.save(&s2).ok();

        let ids = store.list_sessions().ok();
        assert!(ids.is_some());
        let mut ids = ids.unwrap();
        ids.sort();
        assert_eq!(ids, vec!["list-a", "list-b"]);

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_delete_session() {
        let dir = temp_dir();
        let store = JsonFileStore::new(dir.clone());

        let session = DurableSession::new("delete-me");
        store.save(&session).ok();
        assert!(store.load("delete-me").is_ok());

        store.delete("delete-me").ok();
        assert!(store.load("delete-me").is_err());

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_empty_dir() {
        let dir = temp_dir().join("nonexistent-subdir");
        let store = JsonFileStore::new(dir);
        let ids = store.list_sessions().ok();
        assert!(ids.is_some());
        assert!(ids.unwrap().is_empty());
    }
}
