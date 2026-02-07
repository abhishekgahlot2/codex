use std::path::PathBuf;

use chrono::Utc;
use codex_protocol::ThreadId;
use tokio::sync::RwLock;

use crate::error::{Result, TeamError};
use crate::types::{
    TaskStatus, TeamAgent, TeamAgentRole, TeamAgentStatus, TeamMessage, TeamStateData, TeamTask,
};

/// Generate a unique ID with the given prefix, using timestamp + random hex.
fn generate_id(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let r: u32 = rand::random();
    format!("{prefix}-{ts:x}-{r:x}")
}

/// Sanitize a team name so it is safe to use as a filename component.
///
/// - Rejects empty names and the special names "." / "..".
/// - Replaces any character that is **not** alphanumeric, hyphen, or underscore
///   with an underscore.
fn sanitize_team_name(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(TeamError::InvalidOperation(
            "Team name must not be empty".to_string(),
        ));
    }
    if name == "." || name == ".." {
        return Err(TeamError::InvalidOperation(format!(
            "Team name is not allowed: {name}"
        )));
    }
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(sanitized)
}

/// In-memory team state backed by a JSON file on disk.
pub struct TeamState {
    data: RwLock<Option<TeamStateData>>,
    persist_dir: PathBuf,
}

impl TeamState {
    /// Create a new `TeamState` that will persist data under `persist_dir`.
    /// The directory is created lazily on first persist, not at construction time.
    pub fn new(persist_dir: PathBuf) -> Self {
        Self {
            data: RwLock::new(None),
            persist_dir,
        }
    }

    /// If the in-memory state is `None`, scan `persist_dir` for a `.json` file,
    /// deserialize the first one found, and load it into memory.  This allows a
    /// second `TeamState` instance (e.g. a teammate process) to pick up state
    /// that was persisted by a different process.
    async fn try_load_from_disk(&self) {
        // Fast-path: already loaded.
        {
            let guard = self.data.read().await;
            if guard.is_some() {
                return;
            }
        }

        // Scan persist_dir for *.json files.
        let mut read_dir = match tokio::fs::read_dir(&self.persist_dir).await {
            Ok(rd) => rd,
            Err(_) => return, // directory may not exist yet
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    if let Ok(state) = serde_json::from_slice::<TeamStateData>(&bytes) {
                        let mut guard = self.data.write().await;
                        // Double-check: another task may have loaded while we were reading.
                        if guard.is_none() {
                            *guard = Some(state);
                        }
                        return;
                    }
                }
            }
        }
    }

    /// Create a new team with a lead agent.
    pub async fn create_team(&self, team_name: &str, lead_name: &str) -> Result<TeamStateData> {
        // Sanitize the team name before anything else.
        let safe_name = sanitize_team_name(team_name)?;

        let mut guard = self.data.write().await;
        if guard.is_some() {
            return Err(TeamError::InvalidOperation(
                "Team already exists".to_string(),
            ));
        }

        let now = Utc::now();
        let lead_id = generate_id("agent");
        let lead = TeamAgent {
            id: lead_id.clone(),
            name: lead_name.to_string(),
            role: TeamAgentRole::Lead,
            status: TeamAgentStatus::Active,
            model: None,
            thread_id: None,
            created_at: now,
        };

        let state = TeamStateData {
            team: safe_name,
            created_at: now,
            updated_at: now,
            lead_id,
            agents: vec![lead],
            tasks: vec![],
            messages: vec![],
        };

        Self::persist_inner(&self.persist_dir, &state).await?;
        *guard = Some(state.clone());
        Ok(state)
    }

    /// Return a clone of the current team state.
    pub async fn get_team(&self) -> Result<TeamStateData> {
        self.try_load_from_disk().await;
        let guard = self.data.read().await;
        guard.clone().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })
    }

    /// Add an agent to the team.
    pub async fn add_agent(
        &self,
        name: &str,
        role: TeamAgentRole,
        thread_id: Option<codex_protocol::ThreadId>,
        model: Option<String>,
    ) -> Result<TeamAgent> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let agent = TeamAgent {
            id: generate_id("agent"),
            name: name.to_string(),
            role,
            status: TeamAgentStatus::Active,
            model,
            thread_id,
            created_at: Utc::now(),
        };

        state.agents.push(agent.clone());
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(agent)
    }

    /// Bind the lead agent to a concrete thread id.
    ///
    /// This is used to enforce lead-owned lifecycle operations such as cleanup.
    pub async fn bind_lead_thread(&self, thread_id: ThreadId) -> Result<TeamAgent> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let lead = state
            .agents
            .iter_mut()
            .find(|a| a.id == state.lead_id)
            .ok_or_else(|| TeamError::InvalidOperation("Lead agent not found".to_string()))?;

        lead.thread_id = Some(thread_id);
        let result = lead.clone();
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(result)
    }

    /// Add a task. If any dependency IDs are invalid, returns an error.
    /// Tasks with unresolved (non-completed) deps start as `Blocked`; otherwise `Pending`.
    pub async fn add_task(&self, title: &str, depends_on: Vec<String>) -> Result<TeamTask> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        // Validate all dependency IDs exist.
        for dep_id in &depends_on {
            if !state.tasks.iter().any(|t| t.id == *dep_id) {
                return Err(TeamError::InvalidOperation(format!(
                    "Dependency task not found: {dep_id}"
                )));
            }
        }

        // Determine initial status based on whether all deps are completed.
        let all_deps_complete = depends_on.iter().all(|dep_id| {
            state
                .tasks
                .iter()
                .any(|t| t.id == *dep_id && t.status == TaskStatus::Completed)
        });

        let status = if depends_on.is_empty() || all_deps_complete {
            TaskStatus::Pending
        } else {
            TaskStatus::Blocked
        };

        let now = Utc::now();
        let task = TeamTask {
            id: generate_id("task"),
            title: title.to_string(),
            status,
            assignee_id: None,
            depends_on,
            created_at: now,
            updated_at: now,
        };

        state.tasks.push(task.clone());
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(task)
    }

    /// Claim a task for an assignee. The task must not be blocked or completed,
    /// and the assignee must be a member of the team (matched by agent id or name).
    pub async fn claim_task(&self, task_id: &str, assignee_id: &str) -> Result<TeamTask> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        // Validate the assignee is a team member (by id or name).
        if !state
            .agents
            .iter()
            .any(|a| a.id == assignee_id || a.name == assignee_id)
        {
            return Err(TeamError::InvalidOperation(format!(
                "Assignee is not a team member: {assignee_id}"
            )));
        }

        let task = state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| TeamError::InvalidOperation(format!("Task not found: {task_id}")))?;

        if task.status == TaskStatus::Blocked {
            return Err(TeamError::InvalidOperation(
                "Cannot claim a blocked task".to_string(),
            ));
        }
        if task.status == TaskStatus::Completed {
            return Err(TeamError::InvalidOperation(
                "Cannot claim a completed task".to_string(),
            ));
        }

        task.status = TaskStatus::InProgress;
        task.assignee_id = Some(assignee_id.to_string());
        task.updated_at = Utc::now();

        let result = task.clone();
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(result)
    }

    /// Mark a task as completed and auto-unblock dependents whose deps are all done.
    pub async fn complete_task(&self, task_id: &str) -> Result<TeamTask> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let task = state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| TeamError::InvalidOperation(format!("Task not found: {task_id}")))?;

        task.status = TaskStatus::Completed;
        task.updated_at = Utc::now();
        let result = task.clone();

        // Collect all completed task IDs (including the one we just completed).
        let completed_ids: Vec<String> = state
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        // Auto-unblock: for each blocked task, check if all deps are now completed.
        let now = Utc::now();
        for t in &mut state.tasks {
            if t.status == TaskStatus::Blocked
                && !t.depends_on.is_empty()
                && t.depends_on.iter().all(|dep| completed_ids.contains(dep))
            {
                t.status = TaskStatus::Pending;
                t.updated_at = now;
            }
        }

        state.updated_at = now;
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(result)
    }

    /// Return all tasks.
    pub async fn list_tasks(&self) -> Result<Vec<TeamTask>> {
        self.try_load_from_disk().await;
        let guard = self.data.read().await;
        let state = guard.as_ref().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;
        Ok(state.tasks.clone())
    }

    /// Record a message between agents.
    pub async fn send_message(&self, from: &str, to: &str, body: &str) -> Result<TeamMessage> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let msg = TeamMessage {
            id: generate_id("msg"),
            from: from.to_string(),
            to: to.to_string(),
            body: body.to_string(),
            created_at: Utc::now(),
        };

        state.messages.push(msg.clone());
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(msg)
    }

    /// Return messages, optionally limited to the most recent N.
    pub async fn list_messages(&self, limit: Option<usize>) -> Result<Vec<TeamMessage>> {
        self.try_load_from_disk().await;
        let guard = self.data.read().await;
        let state = guard.as_ref().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;
        let msgs = &state.messages;
        match limit {
            Some(n) => {
                let start = msgs.len().saturating_sub(n);
                Ok(msgs[start..].to_vec())
            }
            None => Ok(msgs.clone()),
        }
    }

    /// Update an agent's status (found by ID or name).
    pub async fn update_agent_status(
        &self,
        agent_id: &str,
        status: TeamAgentStatus,
    ) -> Result<TeamAgent> {
        self.try_load_from_disk().await;
        let mut guard = self.data.write().await;
        let state = guard.as_mut().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let agent = state
            .agents
            .iter_mut()
            .find(|a| a.id == agent_id || a.name == agent_id)
            .ok_or_else(|| {
                TeamError::InvalidOperation(format!("Agent not found: {agent_id}"))
            })?;

        agent.status = status;
        let result = agent.clone();
        state.updated_at = Utc::now();
        Self::persist_inner(&self.persist_dir, state).await?;
        Ok(result)
    }

    /// Find an agent by ID or name.
    pub async fn find_agent(&self, name_or_id: &str) -> Result<TeamAgent> {
        self.try_load_from_disk().await;
        let guard = self.data.read().await;
        let state = guard.as_ref().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        state
            .agents
            .iter()
            .find(|a| a.id == name_or_id || a.name == name_or_id)
            .cloned()
            .ok_or_else(|| {
                TeamError::InvalidOperation(format!("Agent not found: {name_or_id}"))
            })
    }

    /// Verify team invariants (for debugging/auditing).
    pub async fn validate_invariants(&self) -> Result<()> {
        let state = self.data.read().await;
        let state = state.as_ref().ok_or(TeamError::InvalidOperation(
            "no team exists".into(),
        ))?;

        // Invariant 1: Exactly one lead
        let leads: Vec<_> = state
            .agents
            .iter()
            .filter(|a| a.role == TeamAgentRole::Lead)
            .collect();
        if leads.len() != 1 {
            return Err(TeamError::InvalidOperation(format!(
                "expected 1 lead, found {}",
                leads.len()
            )));
        }

        // Invariant 2: Lead ID matches team's lead_id
        if leads[0].id != state.lead_id {
            return Err(TeamError::InvalidOperation(
                "lead agent ID mismatch".into(),
            ));
        }

        // Invariant 3: All task dependencies reference existing tasks
        let task_ids: std::collections::HashSet<_> =
            state.tasks.iter().map(|t| t.id.as_str()).collect();
        for task in &state.tasks {
            for dep in &task.depends_on {
                if !task_ids.contains(dep.as_str()) {
                    return Err(TeamError::InvalidOperation(format!(
                        "task '{}' depends on unknown task '{dep}'",
                        task.id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Clear team state and remove the persisted file.
    pub async fn cleanup(&self) -> Result<()> {
        let mut guard = self.data.write().await;
        if let Some(state) = guard.as_ref() {
            let safe_name = sanitize_team_name(&state.team)?;
            let path = self.persist_dir.join(format!("{safe_name}.json"));
            if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                tokio::fs::remove_file(&path).await?;
            }
        }
        *guard = None;
        Ok(())
    }

    /// Validate that cleanup is currently allowed.
    ///
    /// Cleanup is blocked while any non-lead teammate is not shutdown.
    pub async fn assert_cleanup_allowed(&self) -> Result<()> {
        self.try_load_from_disk().await;
        let guard = self.data.read().await;
        let state = guard.as_ref().ok_or_else(|| {
            TeamError::InvalidOperation("No team has been created yet".to_string())
        })?;

        let active_teammates: Vec<&str> = state
            .agents
            .iter()
            .filter(|a| a.role == TeamAgentRole::Teammate && a.status != TeamAgentStatus::Shutdown)
            .map(|a| a.name.as_str())
            .collect();

        if active_teammates.is_empty() {
            return Ok(());
        }

        Err(TeamError::InvalidOperation(format!(
            "Cannot cleanup team while teammates are active: {}. Shut them down first.",
            active_teammates.join(", ")
        )))
    }

    /// Persist the state to disk as JSON. Creates the directory if needed.
    async fn persist_inner(persist_dir: &PathBuf, state: &TeamStateData) -> Result<()> {
        let safe_name = sanitize_team_name(&state.team)?;
        tokio::fs::create_dir_all(persist_dir).await?;
        let path = persist_dir.join(format!("{safe_name}.json"));
        let json = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&path, json.as_bytes()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(dir: &std::path::Path) -> TeamState {
        TeamState::new(dir.to_path_buf())
    }

    #[tokio::test]
    async fn create_team_returns_valid_state() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        let state = ts.create_team("test-team", "lead-agent").await.unwrap();
        assert_eq!(state.team, "test-team");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.agents[0].name, "lead-agent");
        assert_eq!(state.agents[0].role, TeamAgentRole::Lead);
        assert_eq!(state.lead_id, state.agents[0].id);
    }

    #[tokio::test]
    async fn create_team_rejects_duplicate() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("dup-team", "lead").await.unwrap();
        let err = ts.create_team("dup-team", "lead").await.unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn add_agent_after_team_created() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let agent = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();
        assert_eq!(agent.name, "worker");
        assert_eq!(agent.role, TeamAgentRole::Teammate);
        let state = ts.get_team().await.unwrap();
        assert_eq!(state.agents.len(), 2);
    }

    #[tokio::test]
    async fn add_agent_requires_team() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        let err = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("No team"));
    }

    #[tokio::test]
    async fn add_task_with_no_deps_is_pending() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let task = ts.add_task("do something", vec![]).await.unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn add_task_with_unresolved_deps_is_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let t1 = ts.add_task("first", vec![]).await.unwrap();
        let t2 = ts.add_task("second", vec![t1.id.clone()]).await.unwrap();
        assert_eq!(t2.status, TaskStatus::Blocked);
    }

    #[tokio::test]
    async fn add_task_rejects_invalid_dep() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let err = ts
            .add_task("bad", vec!["nonexistent".to_string()])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn claim_task_succeeds_when_unblocked() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let worker = ts
            .add_agent("worker-1", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();
        let task = ts.add_task("claim me", vec![]).await.unwrap();
        let claimed = ts.claim_task(&task.id, &worker.id).await.unwrap();
        assert_eq!(claimed.status, TaskStatus::InProgress);
        assert_eq!(claimed.assignee_id.as_deref(), Some(worker.id.as_str()));
    }

    #[tokio::test]
    async fn claim_task_fails_when_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let worker = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();
        let t1 = ts.add_task("first", vec![]).await.unwrap();
        let t2 = ts.add_task("second", vec![t1.id]).await.unwrap();
        let err = ts.claim_task(&t2.id, &worker.id).await.unwrap_err();
        assert!(err.to_string().contains("blocked"));
    }

    #[tokio::test]
    async fn complete_task_unblocks_dependents() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let t1 = ts.add_task("first", vec![]).await.unwrap();
        let t2 = ts.add_task("second", vec![t1.id.clone()]).await.unwrap();
        assert_eq!(t2.status, TaskStatus::Blocked);

        ts.complete_task(&t1.id).await.unwrap();
        let tasks = ts.list_tasks().await.unwrap();
        let updated_t2 = tasks.iter().find(|t| t.id == t2.id).unwrap();
        assert_eq!(updated_t2.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn send_message_records_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let msg = ts.send_message("alice", "bob", "hello").await.unwrap();
        assert_eq!(msg.from, "alice");
        assert_eq!(msg.to, "bob");
        assert_eq!(msg.body, "hello");
    }

    #[tokio::test]
    async fn list_messages_with_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        for i in 0..5 {
            ts.send_message("a", "b", &format!("msg {i}"))
                .await
                .unwrap();
        }
        let all = ts.list_messages(None).await.unwrap();
        assert_eq!(all.len(), 5);
        let limited = ts.list_messages(Some(2)).await.unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].body, "msg 3");
        assert_eq!(limited[1].body, "msg 4");
    }

    #[tokio::test]
    async fn cleanup_removes_state() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("cleanup-team", "lead").await.unwrap();
        ts.cleanup().await.unwrap();
        assert!(ts.get_team().await.is_err());
        assert!(!tmp.path().join("cleanup-team.json").exists());
    }

    #[tokio::test]
    async fn persistence_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("persist-team", "lead").await.unwrap();
        ts.add_task("a task", vec![]).await.unwrap();

        // Read the file back and verify it round-trips.
        let path = tmp.path().join("persist-team.json");
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        let loaded: TeamStateData = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.team, "persist-team");
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].title, "a task");
    }

    #[tokio::test]
    async fn find_agent_by_name_or_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let agent = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();

        let by_name = ts.find_agent("worker").await.unwrap();
        assert_eq!(by_name.id, agent.id);

        let by_id = ts.find_agent(&agent.id).await.unwrap();
        assert_eq!(by_id.name, "worker");

        assert!(ts.find_agent("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn update_agent_status() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let agent = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();

        let updated = ts
            .update_agent_status(&agent.id, TeamAgentStatus::Idle)
            .await
            .unwrap();
        assert_eq!(updated.status, TeamAgentStatus::Idle);

        // Also find by name.
        let by_name = ts
            .update_agent_status("worker", TeamAgentStatus::Shutdown)
            .await
            .unwrap();
        assert_eq!(by_name.status, TeamAgentStatus::Shutdown);
    }

    #[tokio::test]
    async fn bind_lead_thread_sets_lead_thread_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let thread_id = codex_protocol::ThreadId::new();
        let lead = ts.bind_lead_thread(thread_id).await.unwrap();
        assert_eq!(lead.role, TeamAgentRole::Lead);
        assert_eq!(lead.thread_id, Some(thread_id));
    }

    #[tokio::test]
    async fn cleanup_is_blocked_while_teammates_active() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("cleanup-policy", "lead").await.unwrap();
        ts.add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();

        let err = ts.assert_cleanup_allowed().await.unwrap_err();
        assert!(err
            .to_string()
            .contains("Cannot cleanup team while teammates are active"));
    }

    #[tokio::test]
    async fn cleanup_allowed_after_teammates_shutdown() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("cleanup-policy-ok", "lead").await.unwrap();
        let worker = ts
            .add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();
        ts.update_agent_status(&worker.id, TeamAgentStatus::Shutdown)
            .await
            .unwrap();

        ts.assert_cleanup_allowed().await.unwrap();
    }

    /// Full NL team lifecycle: create team, add agents, manage tasks with
    /// dependencies, send messages, shut down agents, and clean up.
    /// Every step uses only `TeamState` method calls -- no slash commands,
    /// no regex routing -- proving the lifecycle works end-to-end via pure
    /// NL-equivalent function calls.
    #[tokio::test]
    async fn nl_team_lifecycle_full() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());

        // 1. create_team
        let team = ts.create_team("my-project", "lead").await.unwrap();
        assert_eq!(team.team, "my-project");
        assert_eq!(team.agents.len(), 1);
        assert_eq!(team.agents[0].name, "lead");
        assert_eq!(team.agents[0].role, TeamAgentRole::Lead);
        assert_eq!(team.agents[0].status, TeamAgentStatus::Active);
        let lead_id = team.lead_id.clone();

        // 2. add_agent worker-1
        let thread_1 = codex_protocol::ThreadId::new();
        let w1 = ts
            .add_agent(
                "worker-1",
                TeamAgentRole::Teammate,
                Some(thread_1),
                Some("gpt-4".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(w1.name, "worker-1");
        assert_eq!(w1.role, TeamAgentRole::Teammate);
        assert_eq!(w1.status, TeamAgentStatus::Active);
        assert!(w1.thread_id.is_some());
        assert_eq!(w1.model.as_deref(), Some("gpt-4"));

        // 3. add_agent worker-2
        let thread_2 = codex_protocol::ThreadId::new();
        let w2 = ts
            .add_agent(
                "worker-2",
                TeamAgentRole::Teammate,
                Some(thread_2),
                Some("gpt-4".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(w2.name, "worker-2");
        let snapshot = ts.get_team().await.unwrap();
        assert_eq!(snapshot.agents.len(), 3);

        // 4. add_task A (no deps) -> Pending
        let task_a = ts
            .add_task("implement feature A", vec![])
            .await
            .unwrap();
        assert_eq!(task_a.status, TaskStatus::Pending);

        // 5. add_task B (no deps) -> Pending
        let task_b = ts
            .add_task("implement feature B", vec![])
            .await
            .unwrap();
        assert_eq!(task_b.status, TaskStatus::Pending);

        // 6. add_task C (depends on A and B) -> Blocked
        let task_c = ts
            .add_task(
                "integrate A and B",
                vec![task_a.id.clone(), task_b.id.clone()],
            )
            .await
            .unwrap();
        assert_eq!(task_c.status, TaskStatus::Blocked);
        assert_eq!(task_c.depends_on.len(), 2);

        // 7. claim_task A by worker-1 -> InProgress
        let claimed_a = ts.claim_task(&task_a.id, &w1.id).await.unwrap();
        assert_eq!(claimed_a.status, TaskStatus::InProgress);
        assert_eq!(claimed_a.assignee_id.as_deref(), Some(w1.id.as_str()));

        // 8. claim_task B by worker-2 -> InProgress
        let claimed_b = ts.claim_task(&task_b.id, &w2.id).await.unwrap();
        assert_eq!(claimed_b.status, TaskStatus::InProgress);

        // 9. complete_task A -> C still Blocked (B not done)
        ts.complete_task(&task_a.id).await.unwrap();
        let tasks = ts.list_tasks().await.unwrap();
        let c_after_a = tasks.iter().find(|t| t.id == task_c.id).unwrap();
        assert_eq!(c_after_a.status, TaskStatus::Blocked);

        // 10. send_message lead -> worker-1
        let msg = ts
            .send_message(&lead_id, &w1.id, "good work")
            .await
            .unwrap();
        assert_eq!(msg.from, lead_id);
        assert_eq!(msg.to, w1.id);
        assert_eq!(msg.body, "good work");
        assert_eq!(ts.list_messages(None).await.unwrap().len(), 1);

        // 11. complete_task B -> C auto-unblocks to Pending
        ts.complete_task(&task_b.id).await.unwrap();
        let tasks = ts.list_tasks().await.unwrap();
        let c_after_b = tasks.iter().find(|t| t.id == task_c.id).unwrap();
        assert_eq!(c_after_b.status, TaskStatus::Pending);

        // 12. claim_task C by worker-1
        let claimed_c = ts.claim_task(&task_c.id, &w1.id).await.unwrap();
        assert_eq!(claimed_c.status, TaskStatus::InProgress);

        // 13. complete_task C -> all done
        ts.complete_task(&task_c.id).await.unwrap();
        for t in ts.list_tasks().await.unwrap() {
            assert_eq!(t.status, TaskStatus::Completed);
        }

        // 14. shutdown worker-1
        let w1_shut = ts
            .update_agent_status(&w1.id, TeamAgentStatus::Shutdown)
            .await
            .unwrap();
        assert_eq!(w1_shut.status, TeamAgentStatus::Shutdown);

        // 15. shutdown worker-2
        ts.update_agent_status(&w2.id, TeamAgentStatus::Shutdown)
            .await
            .unwrap();
        // lead still active
        let lead = ts.find_agent(&lead_id).await.unwrap();
        assert_eq!(lead.status, TeamAgentStatus::Active);

        // 16. cleanup
        ts.cleanup().await.unwrap();
        assert!(ts.get_team().await.is_err());
        assert!(!tmp.path().join("my-project.json").exists());
    }

    /// Condensed e2e: error paths + edge cases with pure method calls.
    #[tokio::test]
    async fn nl_lifecycle_error_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());

        ts.create_team("e2e", "boss").await.unwrap();
        let boss_id = ts.get_team().await.unwrap().lead_id;

        let alpha = ts
            .add_agent(
                "alpha",
                TeamAgentRole::Teammate,
                Some(codex_protocol::ThreadId::new()),
                None,
            )
            .await
            .unwrap();

        let tx = ts.add_task("task X", vec![]).await.unwrap();
        let ty = ts.add_task("task Y", vec![]).await.unwrap();
        let tz = ts
            .add_task("task Z", vec![tx.id.clone(), ty.id.clone()])
            .await
            .unwrap();

        // Cannot claim blocked task
        let err = ts.claim_task(&tz.id, &alpha.id).await.unwrap_err();
        assert!(err.to_string().contains("blocked"));

        // Complete X and Y to unblock Z
        ts.claim_task(&tx.id, &alpha.id).await.unwrap();
        ts.complete_task(&tx.id).await.unwrap();

        // Cannot claim completed task
        let err = ts.claim_task(&tx.id, &alpha.id).await.unwrap_err();
        assert!(err.to_string().contains("completed"));

        // Messages accumulate
        ts.send_message(&boss_id, &alpha.id, "msg1").await.unwrap();
        ts.send_message(&alpha.id, &boss_id, "msg2").await.unwrap();
        assert_eq!(ts.list_messages(None).await.unwrap().len(), 2);
        assert_eq!(ts.list_messages(Some(1)).await.unwrap().len(), 1);

        // Complete Y -> Z unblocks
        ts.claim_task(&ty.id, &alpha.id).await.unwrap();
        ts.complete_task(&ty.id).await.unwrap();
        let tasks = ts.list_tasks().await.unwrap();
        assert_eq!(
            tasks.iter().find(|t| t.id == tz.id).unwrap().status,
            TaskStatus::Pending
        );

        // find_agent by name
        let found = ts.find_agent("alpha").await.unwrap();
        assert_eq!(found.id, alpha.id);

        // update_agent_status by name
        ts.update_agent_status("alpha", TeamAgentStatus::Shutdown)
            .await
            .unwrap();
        assert_eq!(
            ts.find_agent("alpha").await.unwrap().status,
            TeamAgentStatus::Shutdown
        );

        // Persistence round-trip
        let json_path = tmp.path().join("e2e.json");
        let raw = tokio::fs::read_to_string(&json_path).await.unwrap();
        let disk: TeamStateData = serde_json::from_str(&raw).unwrap();
        assert_eq!(disk.team, "e2e");
        assert_eq!(disk.agents.len(), 2);
        assert_eq!(disk.messages.len(), 2);

        ts.cleanup().await.unwrap();
        assert!(ts.get_team().await.is_err());
    }

    #[tokio::test]
    async fn test_sanitize_team_name() {
        // Rejects empty name.
        assert!(sanitize_team_name("").is_err());

        // Rejects "." and "..".
        assert!(sanitize_team_name(".").is_err());
        assert!(sanitize_team_name("..").is_err());

        // Path traversal characters are replaced with underscores.
        let evil = sanitize_team_name("../../etc/foo").unwrap();
        assert_eq!(evil, "______etc_foo");
        assert!(!evil.contains('/'));
        assert!(!evil.contains('.'));

        // Valid names pass through unchanged.
        assert_eq!(sanitize_team_name("my-team_1").unwrap(), "my-team_1");
        assert_eq!(sanitize_team_name("Alpha123").unwrap(), "Alpha123");

        // Spaces and special chars are replaced.
        assert_eq!(sanitize_team_name("my team!").unwrap(), "my_team_");
    }

    #[tokio::test]
    async fn test_load_from_disk_on_access() {
        let tmp = tempfile::tempdir().unwrap();

        // Create a team with one TeamState instance.
        let ts1 = make_state(tmp.path());
        let created = ts1.create_team("shared-team", "lead").await.unwrap();
        ts1.add_task("disk task", vec![]).await.unwrap();

        // Create a SECOND TeamState pointing at the same directory.
        let ts2 = make_state(tmp.path());

        // The second instance should discover the persisted state automatically.
        let loaded = ts2.get_team().await.unwrap();
        assert_eq!(loaded.team, created.team);
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.agents[0].name, "lead");

        let tasks = ts2.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "disk task");
    }

    #[tokio::test]
    async fn test_claim_task_rejects_invalid_assignee() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("t", "lead").await.unwrap();
        let task = ts.add_task("some task", vec![]).await.unwrap();

        // Try to claim with a string that is neither an agent id nor an agent name.
        let err = ts
            .claim_task(&task.id, "not-a-member")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not a team member"));

        // Claiming by agent name should succeed (the lead is named "lead").
        let claimed = ts.claim_task(&task.id, "lead").await.unwrap();
        assert_eq!(claimed.status, TaskStatus::InProgress);
    }

    #[tokio::test]
    async fn test_validate_invariants_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("inv-team", "lead").await.unwrap();
        ts.add_agent("worker", TeamAgentRole::Teammate, None, None)
            .await
            .unwrap();
        let t1 = ts.add_task("task 1", vec![]).await.unwrap();
        ts.add_task("task 2", vec![t1.id]).await.unwrap();

        // All invariants should hold
        ts.validate_invariants().await.unwrap();
    }

    #[tokio::test]
    async fn test_validate_invariants_multi_lead_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let ts = make_state(tmp.path());
        ts.create_team("bad-team", "lead-1").await.unwrap();

        // Forcefully inject a second lead by adding an agent with Lead role
        ts.add_agent("lead-2", TeamAgentRole::Lead, None, None)
            .await
            .unwrap();

        let err = ts.validate_invariants().await.unwrap_err();
        assert!(err.to_string().contains("expected 1 lead, found 2"));
    }
}
