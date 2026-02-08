use serde::Deserialize;
use serde::Serialize;

/// How a teammate agent is hosted and interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeammateMode {
    /// Teammate runs in the same process as the lead.
    InProcess,
    /// Teammate runs in a separate tmux pane.
    Tmux,
    /// Teammate runs in a separate iTerm tab/window.
    Iterm,
}

impl Default for TeammateMode {
    fn default() -> Self {
        Self::InProcess
    }
}

/// Tracks which agent currently has interactive focus (receives direct user input).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusState {
    /// Agent ID currently focused (None = lead has focus).
    focused_agent: Option<String>,
    /// History of focus changes.
    focus_history: Vec<FocusChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusChange {
    /// Agent that received focus.
    pub agent_id: String,
    /// Timestamp of the focus change.
    pub timestamp: String,
}

impl FocusState {
    pub fn new() -> Self {
        Self {
            focused_agent: None,
            focus_history: Vec::new(),
        }
    }

    /// Switch focus to a specific agent. None = return focus to lead.
    pub fn set_focus(&mut self, agent_id: Option<&str>) {
        self.focused_agent = agent_id.map(|s| s.to_string());
        if let Some(id) = agent_id {
            self.focus_history.push(FocusChange {
                agent_id: id.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }
    }

    /// Get the currently focused agent (None = lead).
    pub fn focused_agent(&self) -> Option<&str> {
        self.focused_agent.as_deref()
    }

    /// Whether a specific agent has focus.
    pub fn has_focus(&self, agent_id: &str) -> bool {
        self.focused_agent.as_deref() == Some(agent_id)
    }

    /// Return focus to the lead.
    pub fn return_to_lead(&mut self) {
        self.focused_agent = None;
    }

    /// Number of focus changes.
    pub fn focus_change_count(&self) -> usize {
        self.focus_history.len()
    }
}

impl Default for FocusState {
    fn default() -> Self {
        Self::new()
    }
}

/// A queued direct message for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    /// Message ID.
    pub id: String,
    /// Sender agent ID.
    pub from: String,
    /// Message body.
    pub body: String,
    /// Whether the message has been delivered/read.
    pub delivered: bool,
    /// Timestamp.
    pub created_at: String,
}

/// Per-agent message inbox.
#[derive(Debug, Clone, Default)]
pub struct MessageInbox {
    messages: Vec<QueuedMessage>,
}

impl MessageInbox {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Queue a message for delivery.
    pub fn enqueue(&mut self, from: &str, body: &str) -> &QueuedMessage {
        let id = format!("msg-{}", self.messages.len() + 1);
        self.messages.push(QueuedMessage {
            id,
            from: from.to_string(),
            body: body.to_string(),
            delivered: false,
            created_at: chrono::Utc::now().to_rfc3339(),
        });
        self.messages.last().unwrap()
    }

    /// Get undelivered messages.
    pub fn undelivered(&self) -> Vec<&QueuedMessage> {
        self.messages.iter().filter(|m| !m.delivered).collect()
    }

    /// Mark a message as delivered.
    pub fn mark_delivered(&mut self, msg_id: &str) -> bool {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == msg_id) {
            msg.delivered = true;
            true
        } else {
            false
        }
    }

    /// Mark all messages as delivered.
    pub fn mark_all_delivered(&mut self) {
        for msg in &mut self.messages {
            msg.delivered = true;
        }
    }

    /// Total message count.
    pub fn total_count(&self) -> usize {
        self.messages.len()
    }

    /// Undelivered count.
    pub fn undelivered_count(&self) -> usize {
        self.messages.iter().filter(|m| !m.delivered).count()
    }
}

/// Combined interaction configuration for a team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionConfig {
    /// Default teammate hosting mode.
    pub default_teammate_mode: TeammateMode,
    /// Whether split-pane direct interaction is enabled.
    pub split_pane_enabled: bool,
    /// Maximum queued messages per agent before oldest are dropped.
    pub max_inbox_size: usize,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            default_teammate_mode: TeammateMode::InProcess,
            split_pane_enabled: false,
            max_inbox_size: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TeammateMode ──────────────────────────────────────────────────

    #[test]
    fn test_default_mode_is_in_process() {
        assert_eq!(TeammateMode::default(), TeammateMode::InProcess);
    }

    #[test]
    fn test_mode_serialization_roundtrip() {
        let variants = [
            TeammateMode::InProcess,
            TeammateMode::Tmux,
            TeammateMode::Iterm,
        ];
        for mode in &variants {
            let json = serde_json::to_string(mode).unwrap();
            let back: TeammateMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, back);
        }
        // Also verify the snake_case naming.
        assert_eq!(
            serde_json::to_string(&TeammateMode::InProcess).unwrap(),
            "\"in_process\""
        );
        assert_eq!(
            serde_json::to_string(&TeammateMode::Tmux).unwrap(),
            "\"tmux\""
        );
        assert_eq!(
            serde_json::to_string(&TeammateMode::Iterm).unwrap(),
            "\"iterm\""
        );
    }

    // ── FocusState ────────────────────────────────────────────────────

    #[test]
    fn test_initial_focus_is_lead() {
        let focus = FocusState::new();
        assert!(focus.focused_agent().is_none());
    }

    #[test]
    fn test_set_and_get_focus() {
        let mut focus = FocusState::new();
        focus.set_focus(Some("agent-1"));
        assert_eq!(focus.focused_agent(), Some("agent-1"));
    }

    #[test]
    fn test_has_focus() {
        let mut focus = FocusState::new();
        focus.set_focus(Some("agent-1"));
        assert!(focus.has_focus("agent-1"));
        assert!(!focus.has_focus("agent-2"));
    }

    #[test]
    fn test_return_to_lead() {
        let mut focus = FocusState::new();
        focus.set_focus(Some("agent-1"));
        assert!(focus.focused_agent().is_some());
        focus.return_to_lead();
        assert!(focus.focused_agent().is_none());
    }

    #[test]
    fn test_focus_history_tracking() {
        let mut focus = FocusState::new();
        assert_eq!(focus.focus_change_count(), 0);

        focus.set_focus(Some("agent-1"));
        assert_eq!(focus.focus_change_count(), 1);

        focus.set_focus(Some("agent-2"));
        assert_eq!(focus.focus_change_count(), 2);

        // Returning to lead (None) does not add a history entry.
        focus.set_focus(None);
        assert_eq!(focus.focus_change_count(), 2);

        focus.set_focus(Some("agent-1"));
        assert_eq!(focus.focus_change_count(), 3);
    }

    // ── MessageInbox ──────────────────────────────────────────────────

    #[test]
    fn test_enqueue_and_retrieve() {
        let mut inbox = MessageInbox::new();
        inbox.enqueue("alice", "hello");
        inbox.enqueue("bob", "world");

        let undelivered = inbox.undelivered();
        assert_eq!(undelivered.len(), 2);
        assert_eq!(undelivered[0].from, "alice");
        assert_eq!(undelivered[0].body, "hello");
        assert_eq!(undelivered[1].from, "bob");
        assert_eq!(undelivered[1].body, "world");
    }

    #[test]
    fn test_mark_delivered() {
        let mut inbox = MessageInbox::new();
        inbox.enqueue("alice", "msg1");
        inbox.enqueue("bob", "msg2");

        assert_eq!(inbox.undelivered_count(), 2);

        let delivered = inbox.mark_delivered("msg-1");
        assert!(delivered);

        let undelivered = inbox.undelivered();
        assert_eq!(undelivered.len(), 1);
        assert_eq!(undelivered[0].from, "bob");

        // Non-existent ID returns false.
        assert!(!inbox.mark_delivered("msg-999"));
    }

    #[test]
    fn test_mark_all_delivered() {
        let mut inbox = MessageInbox::new();
        inbox.enqueue("alice", "msg1");
        inbox.enqueue("bob", "msg2");
        inbox.enqueue("charlie", "msg3");

        assert_eq!(inbox.undelivered_count(), 3);

        inbox.mark_all_delivered();
        assert_eq!(inbox.undelivered_count(), 0);
        assert_eq!(inbox.total_count(), 3);
    }

    #[test]
    fn test_undelivered_count() {
        let mut inbox = MessageInbox::new();
        assert_eq!(inbox.undelivered_count(), 0);
        assert_eq!(inbox.total_count(), 0);

        inbox.enqueue("a", "1");
        inbox.enqueue("b", "2");
        inbox.enqueue("c", "3");
        assert_eq!(inbox.undelivered_count(), 3);
        assert_eq!(inbox.total_count(), 3);

        inbox.mark_delivered("msg-2");
        assert_eq!(inbox.undelivered_count(), 2);
        assert_eq!(inbox.total_count(), 3);
    }

    #[test]
    fn test_inbox_ordering() {
        let mut inbox = MessageInbox::new();
        let bodies = ["first", "second", "third", "fourth"];
        for body in &bodies {
            inbox.enqueue("sender", body);
        }

        let undelivered = inbox.undelivered();
        assert_eq!(undelivered.len(), 4);
        for (i, body) in bodies.iter().enumerate() {
            assert_eq!(undelivered[i].body, *body);
        }
    }

    // ── InteractionConfig ─────────────────────────────────────────────

    #[test]
    fn test_default_config() {
        let config = InteractionConfig::default();
        assert_eq!(config.default_teammate_mode, TeammateMode::InProcess);
        assert!(!config.split_pane_enabled);
        assert_eq!(config.max_inbox_size, 100);
    }

    #[test]
    fn test_config_serialization() {
        let config = InteractionConfig {
            default_teammate_mode: TeammateMode::Tmux,
            split_pane_enabled: true,
            max_inbox_size: 50,
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: InteractionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.default_teammate_mode, TeammateMode::Tmux);
        assert!(back.split_pane_enabled);
        assert_eq!(back.max_inbox_size, 50);
    }
}
