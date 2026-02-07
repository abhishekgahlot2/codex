// --- ConsoleAI team: TeamHandler bridges codex-core internals to console-team state ---
// This is the only file that touches both codex-core pub(crate) types and console-team.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;

pub struct TeamHandler;

#[async_trait]
impl ToolHandler for TeamHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "team handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "team_create" => handle_team_create(session, turn, arguments).await,
            "team_add_task" => handle_team_add_task(session, arguments).await,
            "team_claim_task" => handle_team_claim_task(session, arguments).await,
            "team_complete_task" => handle_team_complete_task(session, arguments).await,
            "team_list_tasks" => handle_team_list_tasks(session).await,
            "team_message" => handle_team_message(session, arguments).await,
            "team_status" => handle_team_status(session).await,
            "team_shutdown_agent" => handle_team_shutdown_agent(session, arguments).await,
            "team_cleanup" => handle_team_cleanup(session).await,
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported team tool: {other}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_output(value: &impl serde::Serialize) -> Result<ToolOutput, FunctionCallError> {
    let content = serde_json::to_string(value)
        .map_err(|e| FunctionCallError::Fatal(format!("failed to serialize team result: {e}")))?;
    Ok(ToolOutput::Function {
        body: codex_protocol::models::FunctionCallOutputBody::Text(content),
        success: Some(true),
    })
}

fn team_err(e: console_team::TeamError) -> FunctionCallError {
    FunctionCallError::RespondToModel(format!("{e}"))
}

// ---------------------------------------------------------------------------
// team_create
// ---------------------------------------------------------------------------

use crate::agent::AgentRole;
use crate::codex::Session;
use crate::codex::TurnContext;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use serde::Deserialize;
use std::process::Command;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct AgentSpec {
    name: String,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TeamCreateArgs {
    team_name: String,
    #[serde(default)]
    agents: Vec<AgentSpec>,
    #[serde(default)]
    teammate_mode: Option<String>,
}

#[derive(Debug, Clone)]
struct SpawnedTeammate {
    name: String,
    thread_id: codex_protocol::ThreadId,
}

fn tmux_mode_enabled(args: &TeamCreateArgs) -> bool {
    let arg_mode = args
        .teammate_mode
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase());
    if matches!(arg_mode.as_deref(), Some("tmux")) {
        return true;
    }

    std::env::var("CONSOLE_TEAMMATE_MODE")
        .map(|v| v.trim().eq_ignore_ascii_case("tmux"))
        .unwrap_or(false)
}

fn ensure_tmux_env() -> Result<(), FunctionCallError> {
    if std::env::var("TMUX").is_ok() {
        Ok(())
    } else {
        Err(FunctionCallError::RespondToModel(
            "teammate_mode=tmux requested, but this session is not running inside tmux".to_string(),
        ))
    }
}

fn run_tmux(args: &[&str]) -> Result<String, FunctionCallError> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|e| FunctionCallError::RespondToModel(format!("tmux command failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(FunctionCallError::RespondToModel(format!(
            "tmux command '{}' failed: {}",
            args.join(" "),
            if stderr.is_empty() {
                "unknown error".to_string()
            } else {
                stderr
            }
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn shell_quote_double(input: &str) -> String {
    format!("\"{}\"", input.replace('\\', "\\\\").replace('"', "\\\""))
}

fn spawn_tmux_panes_for_teammates(teammates: &[SpawnedTeammate]) -> Result<(), FunctionCallError> {
    if teammates.is_empty() {
        return Ok(());
    }

    ensure_tmux_env()?;

    let codex_bin = std::env::var("CONSOLE_CODEX_BIN").unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "codex".to_string())
    });
    let codex_bin_quoted = shell_quote_double(&codex_bin);

    let mut right_anchor: Option<String> = None;
    for (idx, teammate) in teammates.iter().enumerate() {
        let pane_id = if idx == 0 {
            run_tmux(&["split-window", "-h", "-P", "-F", "#{pane_id}"])?
        } else {
            let anchor = right_anchor
                .as_deref()
                .ok_or_else(|| FunctionCallError::Fatal("missing tmux anchor pane".to_string()))?;
            run_tmux(&["split-window", "-v", "-t", anchor, "-P", "-F", "#{pane_id}"])?
        };

        if right_anchor.is_none() {
            right_anchor = Some(pane_id.clone());
        }

        let pane_title = format!("@{}", teammate.name);
        let _ = run_tmux(&["select-pane", "-t", &pane_id, "-T", &pane_title])?;

        let shell_cmd = format!(
            "clear; echo \"teammate: {}\"; echo \"thread: {}\"; {} resume {}",
            teammate.name, teammate.thread_id, codex_bin_quoted, teammate.thread_id
        );
        let _ = run_tmux(&["send-keys", "-t", &pane_id, &shell_cmd, "C-m"])?;
    }

    // Keep the lead pane active.
    let _ = run_tmux(&["last-pane"])?;
    Ok(())
}

async fn handle_team_create(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamCreateArgs = parse_arguments(&arguments)?;

    if args.team_name.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "team_name must not be empty".to_string(),
        ));
    }

    let team_state = &session.services.team_state;

    team_state
        .create_team(&args.team_name, "lead")
        .await
        .map_err(team_err)?;
    team_state
        .bind_lead_thread(session.conversation_id)
        .await
        .map_err(team_err)?;

    // Spawn each requested agent using the existing collab primitives.
    // Track successfully spawned agents so we can roll back on failure.
    let mut spawned_agents = Vec::new();
    let mut spawned_teammates = Vec::new();

    for spec in &args.agents {
        let mut config = (*turn.config).clone();
        if let Some(ref m) = spec.model {
            config.model = Some(String::clone(m));
        }
        AgentRole::Worker
            .apply_to_config(&mut config)
            .map_err(FunctionCallError::RespondToModel)?;

        let prompt = format!(
            "You are a team member named '{}'. Wait for instructions from the team lead.",
            spec.name
        );

        match session
            .services
            .agent_control
            .spawn_agent(
                config,
                prompt,
                Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: session.conversation_id,
                    depth: 1,
                })),
            )
            .await
        {
            Ok(thread_id) => {
                team_state
                    .add_agent(
                        &spec.name,
                        console_team::TeamAgentRole::Teammate,
                        Some(thread_id),
                        spec.model.clone(),
                    )
                    .await
                    .map_err(team_err)?;
                spawned_teammates.push(SpawnedTeammate {
                    name: spec.name.clone(),
                    thread_id: thread_id.clone(),
                });
                spawned_agents.push(thread_id);
            }
            Err(e) => {
                // Rollback: shutdown all previously spawned agents.
                for tid in &spawned_agents {
                    let _ = session.services.agent_control.shutdown_agent(*tid).await;
                }
                // Cleanup the partially-created team state.
                let _ = team_state.cleanup().await;
                return Err(FunctionCallError::RespondToModel(format!(
                    "failed to spawn agent '{}': {e} (team rolled back)",
                    spec.name
                )));
            }
        }
    }

    if tmux_mode_enabled(&args) {
        spawn_tmux_panes_for_teammates(&spawned_teammates)?;
    }

    let team = team_state.get_team().await.map_err(team_err)?;
    json_output(&team)
}

// ---------------------------------------------------------------------------
// team_add_task
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamAddTaskArgs {
    title: String,
    #[serde(default)]
    depends_on: Vec<String>,
}

async fn handle_team_add_task(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamAddTaskArgs = parse_arguments(&arguments)?;
    let task = session
        .services
        .team_state
        .add_task(&args.title, args.depends_on)
        .await
        .map_err(team_err)?;
    json_output(&task)
}

// ---------------------------------------------------------------------------
// team_claim_task
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamClaimTaskArgs {
    task_id: String,
    assignee_id: String,
}

async fn handle_team_claim_task(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamClaimTaskArgs = parse_arguments(&arguments)?;
    let task = session
        .services
        .team_state
        .claim_task(&args.task_id, &args.assignee_id)
        .await
        .map_err(team_err)?;
    json_output(&task)
}

// ---------------------------------------------------------------------------
// team_complete_task
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamCompleteTaskArgs {
    task_id: String,
}

async fn handle_team_complete_task(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamCompleteTaskArgs = parse_arguments(&arguments)?;
    let task = session
        .services
        .team_state
        .complete_task(&args.task_id)
        .await
        .map_err(team_err)?;
    json_output(&task)
}

// ---------------------------------------------------------------------------
// team_list_tasks
// ---------------------------------------------------------------------------

async fn handle_team_list_tasks(session: Arc<Session>) -> Result<ToolOutput, FunctionCallError> {
    let tasks = session
        .services
        .team_state
        .list_tasks()
        .await
        .map_err(team_err)?;
    json_output(&tasks)
}

// ---------------------------------------------------------------------------
// team_message
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamMessageArgs {
    #[serde(default)]
    from: Option<String>,
    to: String,
    body: String,
}

async fn handle_team_message(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamMessageArgs = parse_arguments(&arguments)?;

    if args.body.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "message body must not be empty".to_string(),
        ));
    }

    let team_state = &session.services.team_state;

    let recipient = team_state.find_agent(&args.to).await.map_err(team_err)?;

    // Send via collab primitives if the agent has a thread.
    if let Some(thread_id) = recipient.thread_id {
        session
            .services
            .agent_control
            .send_prompt(thread_id, args.body.clone())
            .await
            .map_err(|e| {
                FunctionCallError::RespondToModel(format!(
                    "failed to send message to '{}': {e}",
                    args.to
                ))
            })?;
    }

    let team = team_state.get_team().await.map_err(team_err)?;
    let sender_id = if let Some(ref from) = args.from {
        let sender = team_state.find_agent(from).await.map_err(team_err)?;
        sender.id
    } else {
        team.lead_id.clone()
    };
    let msg = team_state
        .send_message(&sender_id, &recipient.id, &args.body)
        .await
        .map_err(team_err)?;

    json_output(&msg)
}

// ---------------------------------------------------------------------------
// team_status
// ---------------------------------------------------------------------------

async fn handle_team_status(session: Arc<Session>) -> Result<ToolOutput, FunctionCallError> {
    let team = session
        .services
        .team_state
        .get_team()
        .await
        .map_err(team_err)?;
    json_output(&team)
}

// ---------------------------------------------------------------------------
// team_shutdown_agent
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamShutdownAgentArgs {
    agent_id: String,
}

async fn handle_team_shutdown_agent(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamShutdownAgentArgs = parse_arguments(&arguments)?;
    let team_state = &session.services.team_state;
    let team = team_state.get_team().await.map_err(team_err)?;

    let agent = team_state
        .find_agent(&args.agent_id)
        .await
        .map_err(team_err)?;

    if agent.id == team.lead_id {
        return Err(FunctionCallError::RespondToModel(
            "Cannot shut down the lead agent".to_string(),
        ));
    }

    if let Some(thread_id) = agent.thread_id {
        let _ = session
            .services
            .agent_control
            .shutdown_agent(thread_id)
            .await;
    }

    team_state
        .update_agent_status(&agent.id, console_team::TeamAgentStatus::Shutdown)
        .await
        .map_err(team_err)?;

    json_output(&serde_json::json!({
        "agent_id": agent.id,
        "name": agent.name,
        "status": "shutdown"
    }))
}

// ---------------------------------------------------------------------------
// team_cleanup
// ---------------------------------------------------------------------------

async fn handle_team_cleanup(session: Arc<Session>) -> Result<ToolOutput, FunctionCallError> {
    let team_state = &session.services.team_state;
    let team = team_state.get_team().await.map_err(team_err)?;

    // Lead-only cleanup ownership: only the lead thread can initiate team cleanup.
    let lead = team
        .agents
        .iter()
        .find(|a| a.id == team.lead_id)
        .ok_or_else(|| FunctionCallError::RespondToModel("Lead agent not found".to_string()))?;

    if let Some(lead_thread) = lead.thread_id
        && lead_thread != session.conversation_id
    {
        return Err(FunctionCallError::RespondToModel(
            "Cleanup must be initiated by the team lead session".to_string(),
        ));
    }

    // Claude-parity semantics: cleanup fails until teammates are shut down.
    team_state
        .assert_cleanup_allowed()
        .await
        .map_err(team_err)?;
    team_state.cleanup().await.map_err(team_err)?;

    json_output(&serde_json::json!({
        "team": team.team,
        "status": "cleaned_up",
        "agents_shutdown": 0
    }))
}
