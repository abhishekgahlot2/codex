use serde::{Deserialize, Serialize};

/// Controls how a teammate executes assigned work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegateMode {
    /// Teammate executes freely without approval gates.
    Full,
    /// Teammate must submit a plan for lead approval before executing.
    PlanApproval,
    /// Teammate waits for explicit step-by-step instructions.
    Manual,
}

impl Default for DelegateMode {
    fn default() -> Self {
        Self::Full
    }
}

/// Policy controlling delegation behavior for a team.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatePolicy {
    /// Default delegation mode for new teammates.
    pub default_mode: DelegateMode,
    /// Whether teammates can override their delegation mode.
    pub allow_mode_override: bool,
    /// Maximum plan submissions before auto-rejection.
    pub max_plan_revisions: u32,
}

impl Default for DelegatePolicy {
    fn default() -> Self {
        Self {
            default_mode: DelegateMode::Full,
            allow_mode_override: false,
            max_plan_revisions: 3,
        }
    }
}

/// Status of a plan submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Waiting for lead review.
    Pending,
    /// Approved by lead.
    Approved,
    /// Rejected with feedback.
    Rejected,
}

/// A plan submitted by a teammate for approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSubmission {
    /// Unique plan ID.
    pub id: String,
    /// Agent who submitted the plan.
    pub agent_id: String,
    /// Task this plan is for (optional).
    pub task_id: Option<String>,
    /// The plan text.
    pub plan_text: String,
    /// Current status.
    pub status: PlanStatus,
    /// Lead feedback (if rejected).
    pub feedback: Option<String>,
    /// Revision number (starts at 1).
    pub revision: u32,
    /// Timestamp.
    pub created_at: String,
}

/// Tracks plan approval state for a team.
#[derive(Debug, Clone, Default)]
pub struct PlanApprovalState {
    submissions: Vec<PlanSubmission>,
}

impl PlanApprovalState {
    pub fn new() -> Self {
        Self {
            submissions: Vec::new(),
        }
    }

    /// Submit a new plan for approval.
    pub fn submit_plan(
        &mut self,
        agent_id: &str,
        task_id: Option<&str>,
        plan_text: &str,
        policy: &DelegatePolicy,
    ) -> Result<&PlanSubmission, String> {
        // Count existing revisions for this agent+task combo
        let revision_count = self
            .submissions
            .iter()
            .filter(|s| s.agent_id == agent_id && s.task_id.as_deref() == task_id)
            .count() as u32;

        if revision_count >= policy.max_plan_revisions {
            return Err(format!(
                "max plan revisions ({}) reached for agent '{agent_id}'",
                policy.max_plan_revisions
            ));
        }

        let id = format!("plan-{}-{}", agent_id, revision_count + 1);
        let submission = PlanSubmission {
            id,
            agent_id: agent_id.to_string(),
            task_id: task_id.map(|s| s.to_string()),
            plan_text: plan_text.to_string(),
            status: PlanStatus::Pending,
            feedback: None,
            revision: revision_count + 1,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.submissions.push(submission);
        Ok(self.submissions.last().unwrap())
    }

    /// Approve a pending plan.
    pub fn approve_plan(&mut self, plan_id: &str) -> Result<&PlanSubmission, String> {
        let plan = self
            .submissions
            .iter_mut()
            .find(|s| s.id == plan_id)
            .ok_or_else(|| format!("plan '{plan_id}' not found"))?;

        if plan.status != PlanStatus::Pending {
            return Err(format!(
                "plan '{plan_id}' is not pending (status: {:?})",
                plan.status
            ));
        }
        plan.status = PlanStatus::Approved;
        Ok(plan)
    }

    /// Reject a pending plan with feedback.
    pub fn reject_plan(
        &mut self,
        plan_id: &str,
        feedback: &str,
    ) -> Result<&PlanSubmission, String> {
        let plan = self
            .submissions
            .iter_mut()
            .find(|s| s.id == plan_id)
            .ok_or_else(|| format!("plan '{plan_id}' not found"))?;

        if plan.status != PlanStatus::Pending {
            return Err(format!(
                "plan '{plan_id}' is not pending (status: {:?})",
                plan.status
            ));
        }
        plan.status = PlanStatus::Rejected;
        plan.feedback = Some(feedback.to_string());
        Ok(plan)
    }

    /// Get pending plans for a specific agent.
    pub fn pending_plans_for(&self, agent_id: &str) -> Vec<&PlanSubmission> {
        self.submissions
            .iter()
            .filter(|s| s.agent_id == agent_id && s.status == PlanStatus::Pending)
            .collect()
    }

    /// Get all submissions.
    pub fn all_submissions(&self) -> &[PlanSubmission] {
        &self.submissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delegate_mode_default() {
        let mode = DelegateMode::default();
        assert_eq!(mode, DelegateMode::Full);
    }

    #[test]
    fn test_delegate_policy_default() {
        let policy = DelegatePolicy::default();
        assert_eq!(policy.default_mode, DelegateMode::Full);
        assert!(!policy.allow_mode_override);
        assert_eq!(policy.max_plan_revisions, 3);
    }

    #[test]
    fn test_submit_plan() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy::default();
        let plan = state
            .submit_plan("agent-1", Some("task-1"), "my plan", &policy)
            .unwrap();
        assert_eq!(plan.status, PlanStatus::Pending);
        assert_eq!(plan.agent_id, "agent-1");
        assert_eq!(plan.task_id.as_deref(), Some("task-1"));
        assert_eq!(plan.plan_text, "my plan");
        assert_eq!(plan.revision, 1);
        assert!(plan.feedback.is_none());
    }

    #[test]
    fn test_approve_plan() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy::default();
        let plan = state
            .submit_plan("agent-1", None, "plan text", &policy)
            .unwrap();
        let plan_id = plan.id.clone();

        let approved = state.approve_plan(&plan_id).unwrap();
        assert_eq!(approved.status, PlanStatus::Approved);
    }

    #[test]
    fn test_reject_plan_with_feedback() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy::default();
        let plan = state
            .submit_plan("agent-1", None, "plan text", &policy)
            .unwrap();
        let plan_id = plan.id.clone();

        let rejected = state.reject_plan(&plan_id, "needs more detail").unwrap();
        assert_eq!(rejected.status, PlanStatus::Rejected);
        assert_eq!(rejected.feedback.as_deref(), Some("needs more detail"));
    }

    #[test]
    fn test_max_revisions_enforced() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy {
            max_plan_revisions: 2,
            ..DelegatePolicy::default()
        };

        state
            .submit_plan("agent-1", Some("task-1"), "plan v1", &policy)
            .unwrap();
        state
            .submit_plan("agent-1", Some("task-1"), "plan v2", &policy)
            .unwrap();

        let err = state
            .submit_plan("agent-1", Some("task-1"), "plan v3", &policy)
            .unwrap_err();
        assert!(err.contains("max plan revisions"));
        assert!(err.contains("2"));
    }

    #[test]
    fn test_approve_non_pending_fails() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy::default();

        let plan = state
            .submit_plan("agent-1", None, "plan text", &policy)
            .unwrap();
        let plan_id = plan.id.clone();

        // Approve it first
        state.approve_plan(&plan_id).unwrap();

        // Trying to approve again should fail
        let err = state.approve_plan(&plan_id).unwrap_err();
        assert!(err.contains("not pending"));

        // Submit another and reject it
        let plan2 = state
            .submit_plan("agent-2", None, "plan text 2", &policy)
            .unwrap();
        let plan2_id = plan2.id.clone();
        state.reject_plan(&plan2_id, "no good").unwrap();

        // Trying to approve a rejected plan should fail
        let err = state.approve_plan(&plan2_id).unwrap_err();
        assert!(err.contains("not pending"));
    }

    #[test]
    fn test_pending_plans_filter() {
        let mut state = PlanApprovalState::new();
        let policy = DelegatePolicy::default();

        let p1 = state
            .submit_plan("agent-1", None, "plan 1", &policy)
            .unwrap();
        let p1_id = p1.id.clone();

        state
            .submit_plan("agent-1", Some("task-2"), "plan 2", &policy)
            .unwrap();

        state
            .submit_plan("agent-2", None, "other agent plan", &policy)
            .unwrap();

        // Approve the first plan for agent-1
        state.approve_plan(&p1_id).unwrap();

        // Only one pending plan remains for agent-1
        let pending = state.pending_plans_for("agent-1");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].plan_text, "plan 2");

        // agent-2 has one pending plan
        let pending_2 = state.pending_plans_for("agent-2");
        assert_eq!(pending_2.len(), 1);
    }

    #[test]
    fn test_plan_status_serialization() {
        let statuses = [PlanStatus::Pending, PlanStatus::Approved, PlanStatus::Rejected];
        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let deserialized: PlanStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*status, deserialized);
        }

        // Verify snake_case naming
        assert_eq!(serde_json::to_string(&PlanStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&PlanStatus::Approved).unwrap(), "\"approved\"");
        assert_eq!(serde_json::to_string(&PlanStatus::Rejected).unwrap(), "\"rejected\"");
    }

    #[test]
    fn test_delegate_mode_serialization() {
        let modes = [DelegateMode::Full, DelegateMode::PlanApproval, DelegateMode::Manual];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let deserialized: DelegateMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, deserialized);
        }

        // Verify snake_case naming
        assert_eq!(serde_json::to_string(&DelegateMode::Full).unwrap(), "\"full\"");
        assert_eq!(
            serde_json::to_string(&DelegateMode::PlanApproval).unwrap(),
            "\"plan_approval\""
        );
        assert_eq!(serde_json::to_string(&DelegateMode::Manual).unwrap(), "\"manual\"");
    }
}
