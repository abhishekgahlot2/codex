use serde::{Deserialize, Serialize};

/// A checkpoint captures session state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub label: Option<String>,
    pub message_count: usize,
    pub token_count: u64,
    pub cost_usd: f64,
    pub code_snapshot_hash: Option<String>,
    pub created_at: String,
}

/// What to restore from a checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointAction {
    /// Restore both code and conversation state.
    RestoreAll,
    /// Restore only conversation (messages, tasks, team).
    RestoreConversation,
    /// Restore only code (git checkout to snapshot hash).
    RestoreCode,
    /// Create a summary from a selection of messages.
    Summarize,
}

/// Manages checkpoints for a session.
#[derive(Debug, Clone, Default)]
pub struct CheckpointManager {
    checkpoints: Vec<Checkpoint>,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: Vec::new(),
        }
    }

    pub fn create_checkpoint(
        &mut self,
        session_id: &str,
        message_count: usize,
        token_count: u64,
        cost_usd: f64,
        code_hash: Option<&str>,
    ) -> &Checkpoint {
        let id = format!("cp-{}", self.checkpoints.len() + 1);
        self.checkpoints.push(Checkpoint {
            id,
            session_id: session_id.into(),
            label: None,
            message_count,
            token_count,
            cost_usd,
            code_snapshot_hash: code_hash.map(|s| s.into()),
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        self.checkpoints.last().unwrap()
    }

    pub fn get(&self, checkpoint_id: &str) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == checkpoint_id)
    }

    pub fn list(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    pub fn latest(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_checkpoint() {
        let mut mgr = CheckpointManager::new();
        let cp = mgr.create_checkpoint("sess-1", 10, 5000, 0.05, Some("abc123"));
        assert_eq!(cp.id, "cp-1");
        assert_eq!(cp.session_id, "sess-1");
        assert_eq!(cp.message_count, 10);
        assert_eq!(cp.token_count, 5000);
        assert_eq!(cp.code_snapshot_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_list_checkpoints() {
        let mut mgr = CheckpointManager::new();
        mgr.create_checkpoint("sess-1", 5, 1000, 0.01, None);
        mgr.create_checkpoint("sess-1", 10, 2000, 0.02, None);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_get_by_id() {
        let mut mgr = CheckpointManager::new();
        mgr.create_checkpoint("sess-1", 5, 1000, 0.01, None);
        mgr.create_checkpoint("sess-1", 10, 2000, 0.02, None);

        let cp = mgr.get("cp-2");
        assert!(cp.is_some());
        assert_eq!(cp.unwrap().message_count, 10);

        assert!(mgr.get("cp-99").is_none());
    }

    #[test]
    fn test_latest() {
        let mut mgr = CheckpointManager::new();
        assert!(mgr.latest().is_none());

        mgr.create_checkpoint("sess-1", 5, 1000, 0.01, None);
        mgr.create_checkpoint("sess-1", 15, 3000, 0.03, None);

        let latest = mgr.latest();
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().message_count, 15);
    }

    #[test]
    fn test_checkpoint_action_serialization() {
        let action = CheckpointAction::RestoreAll;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"restore_all\"");

        let deserialized: CheckpointAction = serde_json::from_str("\"restore_code\"").unwrap();
        assert_eq!(deserialized, CheckpointAction::RestoreCode);

        let summarize: CheckpointAction = serde_json::from_str("\"summarize\"").unwrap();
        assert_eq!(summarize, CheckpointAction::Summarize);

        let conversation: CheckpointAction =
            serde_json::from_str("\"restore_conversation\"").unwrap();
        assert_eq!(conversation, CheckpointAction::RestoreConversation);
    }
}
