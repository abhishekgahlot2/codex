// --- ConsoleAI team: TeamHandler bridges codex-core internals to console-team state ---
// This is the only file that touches both codex-core pub(crate) types and console-team.
//
// Architecture (two modes):
//   Tmux mode (default when $TMUX set): each teammate is a real `codex --full-auto`
//   process running in its own tmux pane. No thread_id in TeamState. Messages
//   delivered via `tmux send-keys`, shutdown via `tmux kill-pane`.
//
//   In-process mode: teammates are collab sub-agents with thread_ids, managed
//   via agent_control.spawn_agent / send_prompt / shutdown_agent.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use console_tui::{agent_env_vars, format_agent_tree, pane_header_shell_cmd};

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
            "team_broadcast" => handle_team_broadcast(session, arguments).await,
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
use std::collections::HashSet;
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

fn tmux_mode_enabled(args: &TeamCreateArgs) -> bool {
    let arg_mode = args
        .teammate_mode
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase().replace('_', "-"));
    let env_mode = std::env::var("CONSOLE_TEAMMATE_MODE")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase().replace('_', "-"));
    let resolved = arg_mode
        .or(env_mode)
        .unwrap_or_else(|| "auto".to_string());

    match resolved.as_str() {
        "tmux" => true,
        "in-process" => false,
        // Claude-style default: if already in tmux, use split panes.
        "auto" => std::env::var("TMUX").is_ok(),
        // Unknown mode values should not disable teammate panes in tmux sessions.
        _ => std::env::var("TMUX").is_ok(),
    }
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

fn codex_bin_path() -> String {
    std::env::var("CONSOLE_CODEX_BIN").unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "codex".to_string())
    })
}

/// Spawn tmux panes where each pane runs a real codex process (the actual
/// agent), not a viewer. The teammate prompt is passed via `--prompt` so the
/// codex instance starts working immediately.
fn spawn_tmux_agent_panes(
    team_name: &str,
    agents: &[AgentSpec],
) -> Result<(), FunctionCallError> {
    if agents.is_empty() {
        return Ok(());
    }

    ensure_tmux_env()?;

    let codex_bin = codex_bin_path();
    let codex_bin_quoted = shell_quote_double(&codex_bin);

    let existing_titles_output =
        run_tmux(&["list-panes", "-a", "-F", "#{pane_title}"]).unwrap_or_default();
    let existing_titles: HashSet<String> = existing_titles_output
        .lines()
        .map(|line| line.trim().to_string())
        .collect();

    let agents_to_spawn: Vec<&AgentSpec> = agents
        .iter()
        .filter(|a| !existing_titles.contains(&format!("@{}", a.name)))
        .collect();
    if agents_to_spawn.is_empty() {
        return Ok(());
    }

    let mut right_anchor: Option<String> = None;
    for (idx, spec) in agents_to_spawn.iter().enumerate() {
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

        let pane_title = format!("@{}", spec.name);
        let _ = run_tmux(&["select-pane", "-t", &pane_id, "-T", &pane_title]);

        // Print a colored header bar before launching codex.
        let header_cmd = pane_header_shell_cmd(&spec.name, idx);
        let _ = run_tmux(&["send-keys", "-t", &pane_id, &header_cmd, "C-m"]);

        // Build the teammate prompt. The codex process is the actual agent.
        let teammate_prompt = format!(
            "You are team member '{}' on team '{}'. \
Use team_list_tasks to find available tasks. \
Use team_claim_task to claim work. \
When done, call team_complete_task with the result field containing your output. \
The lead reads results directly from the task board — do NOT use team_message \
to send results to the lead. Use team_message only to ask questions. \
Start by checking for available tasks.",
            spec.name, team_name
        );
        let escaped_prompt = shell_quote_double(&teammate_prompt);

        // Set agent env vars for the teammate process, then launch codex.
        let env_vars = agent_env_vars(&spec.name, team_name, idx);
        let env_prefix: String = env_vars
            .iter()
            .map(|(k, v)| format!("{k}={}", shell_quote_double(v)))
            .collect::<Vec<_>>()
            .join(" ");

        // The codex CLI takes the prompt as a positional argument, not --prompt.
        // Also pass --full-auto so the teammate doesn't block on confirmations.
        let shell_cmd = format!(
            "{env_prefix} {codex_bin_quoted} --full-auto {escaped_prompt}",
        );
        let _ = run_tmux(&["send-keys", "-t", &pane_id, &shell_cmd, "C-m"]);
    }

    // Keep the lead pane active.
    let _ = run_tmux(&["last-pane"]);
    Ok(())
}

/// Find a tmux pane ID by its title (e.g. "@worker-1").
#[allow(dead_code)]
fn find_pane_by_title(title: &str) -> Option<String> {
    let panes = run_tmux(&["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"]).ok()?;
    for line in panes.lines() {
        let mut parts = line.splitn(2, '\t');
        let pane_id = parts.next()?.trim();
        let pane_title = parts.next().unwrap_or_default().trim();
        if pane_title == title {
            return Some(pane_id.to_string());
        }
    }
    None
}

/// Deliver a message to a teammate by typing it into their tmux pane.
#[allow(dead_code)]
fn send_message_to_pane(agent_name: &str, message: &str) -> Result<(), FunctionCallError> {
    let title = format!("@{agent_name}");
    let pane_id = find_pane_by_title(&title).ok_or_else(|| {
        FunctionCallError::RespondToModel(format!(
            "no tmux pane found for teammate '{agent_name}'"
        ))
    })?;
    // Type the message into the pane's codex prompt and press Enter.
    let escaped = message.replace('\'', "'\\''");
    run_tmux(&["send-keys", "-t", &pane_id, &escaped, "Enter"])?;
    Ok(())
}

fn close_tmux_panes_for_agent_names(agent_names: &[String]) -> Result<usize, FunctionCallError> {
    if agent_names.is_empty() || std::env::var("TMUX").is_err() {
        return Ok(0);
    }

    let target_titles: HashSet<String> = agent_names.iter().map(|n| format!("@{n}")).collect();
    let panes = match run_tmux(&["list-panes", "-a", "-F", "#{pane_id}\t#{pane_title}"]) {
        Ok(output) => output,
        Err(_) => return Ok(0),
    };

    let mut closed = 0usize;
    for line in panes.lines() {
        let mut parts = line.splitn(2, '\t');
        let pane_id = match parts.next() {
            Some(id) if !id.trim().is_empty() => id.trim(),
            _ => continue,
        };
        let pane_title = parts.next().unwrap_or_default().trim();
        if target_titles.contains(pane_title)
            && run_tmux(&["kill-pane", "-t", pane_id]).is_ok()
        {
            closed += 1;
        }
    }

    Ok(closed)
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

    // Reuse existing team when names match, and still ensure teammate panes are visible in tmux.
    if let Ok(existing_team) = team_state.get_team().await {
        if existing_team.team == args.team_name {
            if tmux_mode_enabled(&args) {
                // Re-open panes for any active teammates that don't have one yet.
                let active_specs: Vec<AgentSpec> = existing_team
                    .agents
                    .iter()
                    .filter(|a| {
                        a.role == console_team::TeamAgentRole::Teammate
                            && a.status != console_team::TeamAgentStatus::Shutdown
                    })
                    .map(|a| AgentSpec {
                        name: a.name.clone(),
                        model: a.model.clone(),
                    })
                    .collect();
                spawn_tmux_agent_panes(&existing_team.team, &active_specs)?;
            }
            return json_output(&existing_team);
        }
        return Err(FunctionCallError::RespondToModel(format!(
            "team '{}' already exists. Clean it up before creating '{}'.",
            existing_team.team, args.team_name
        )));
    }

    team_state
        .create_team(&args.team_name, "lead")
        .await
        .map_err(team_err)?;
    team_state
        .bind_lead_thread(session.conversation_id)
        .await
        .map_err(team_err)?;

    let use_tmux = tmux_mode_enabled(&args);

    if use_tmux {
        // Set the lead pane title so teammates can deliver messages via tmux.
        let _ = run_tmux(&["select-pane", "-T", "@lead"]);

        // Tmux mode: each pane IS the agent (real codex process).
        // Register agents in state without thread_ids, then spawn panes.
        for spec in &args.agents {
            team_state
                .add_agent(
                    &spec.name,
                    console_team::TeamAgentRole::Teammate,
                    None,
                    spec.model.clone(),
                )
                .await
                .map_err(team_err)?;
        }
        if let Err(e) = spawn_tmux_agent_panes(&args.team_name, &args.agents) {
            // Rollback: close any panes we managed to open, then clean state.
            let names: Vec<String> = args.agents.iter().map(|a| a.name.clone()).collect();
            let _ = close_tmux_panes_for_agent_names(&names);
            let _ = team_state.cleanup().await;
            return Err(e);
        }
    } else {
        // In-process mode: spawn collab agents with thread_ids.
        let mut spawned_agents = Vec::new();
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
                    spawned_agents.push(thread_id);
                }
                Err(e) => {
                    for tid in &spawned_agents {
                        let _ = session.services.agent_control.shutdown_agent(*tid).await;
                    }
                    let _ = team_state.cleanup().await;
                    return Err(FunctionCallError::RespondToModel(format!(
                        "failed to spawn agent '{}': {e} (team rolled back)",
                        spec.name
                    )));
                }
            }
        }
    }

    let team = team_state.get_team().await.map_err(team_err)?;

    // Build a Claude Code-style agent tree for model/user display.
    let agent_tree_items: Vec<(String, Option<String>)> = team
        .agents
        .iter()
        .filter(|a| a.role == console_team::TeamAgentRole::Teammate)
        .map(|a| (a.name.clone(), None))
        .collect();
    let tree_view = format_agent_tree(&agent_tree_items);

    // Return both the JSON state and the pretty tree view.
    let mut result = serde_json::to_value(&team)
        .map_err(|e| FunctionCallError::Fatal(format!("failed to serialize team: {e}")))?;
    if let serde_json::Value::Object(ref mut map) = result {
        map.insert("display".into(), serde_json::Value::String(tree_view));
    }
    json_output(&result)
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
    /// Optional result / output text to attach to the completed task.
    /// The lead reads this from the task board via team_list_tasks.
    #[serde(default)]
    result: Option<String>,
}

async fn handle_team_complete_task(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamCompleteTaskArgs = parse_arguments(&arguments)?;
    let task = session
        .services
        .team_state
        .complete_task(&args.task_id, args.result)
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

    // Build a Claude Code-style checklist alongside the raw JSON.
    let display_items: Vec<console_tui::TaskDisplayItem> = tasks
        .iter()
        .map(|t| {
            let status = match t.status {
                console_team::TaskStatus::Pending => {
                    console_tui::TaskDisplayStatus::Pending
                }
                console_team::TaskStatus::InProgress => {
                    console_tui::TaskDisplayStatus::InProgress
                }
                console_team::TaskStatus::Completed => {
                    console_tui::TaskDisplayStatus::Completed
                }
                console_team::TaskStatus::Blocked => {
                    console_tui::TaskDisplayStatus::Blocked
                }
            };
            console_tui::TaskDisplayItem {
                title: t.title.clone(),
                status,
                assignee: t.assignee_id.clone(),
            }
        })
        .collect();
    let checklist = console_tui::format_task_checklist(&display_items);

    let result = serde_json::json!({
        "tasks": tasks,
        "display": checklist,
    });
    json_output(&result)
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

    let team = team_state.get_team().await.map_err(team_err)?;

    // Deliver the message.
    // In tmux mode, messages are persisted to shared state only — no
    // send-keys delivery.  Recipients read messages via team_list_tasks /
    // team_status.  This avoids injecting text into pane input prompts.
    // In-process mode, deliver via collab send_prompt.
    if std::env::var("TMUX").is_err() {
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
    }
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
// team_broadcast
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TeamBroadcastArgs {
    #[serde(default)]
    from: Option<String>,
    body: String,
}

async fn handle_team_broadcast(
    session: Arc<Session>,
    arguments: String,
) -> Result<ToolOutput, FunctionCallError> {
    let args: TeamBroadcastArgs = parse_arguments(&arguments)?;

    if args.body.trim().is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "broadcast body must not be empty".to_string(),
        ));
    }

    let team_state = &session.services.team_state;
    let team = team_state.get_team().await.map_err(team_err)?;

    let sender_id = if let Some(ref from) = args.from {
        let sender = team_state.find_agent(from).await.map_err(team_err)?;
        sender.id
    } else {
        team.lead_id.clone()
    };

    // Broadcast via state (persists messages).
    let messages = team_state
        .broadcast_message(&sender_id, &args.body)
        .await
        .map_err(team_err)?;

    // In tmux mode, messages are persisted to shared state only.
    // Recipients read them via team_list_tasks / team_status.
    // In-process mode would deliver via send_prompt (not yet wired for broadcast).

    json_output(&serde_json::json!({
        "broadcast": true,
        "recipients": messages.len(),
        "body": args.body,
    }))
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
        // In-process mode: shut down the collab thread.
        let _ = session
            .services
            .agent_control
            .shutdown_agent(thread_id)
            .await;
    }

    // Tmux mode: kill the pane (which kills the codex process inside it).
    let panes_closed =
        close_tmux_panes_for_agent_names(std::slice::from_ref(&agent.name)).unwrap_or(0);

    team_state
        .update_agent_status(&agent.id, console_team::TeamAgentStatus::Shutdown)
        .await
        .map_err(team_err)?;

    json_output(&serde_json::json!({
        "agent_id": agent.id,
        "name": agent.name,
        "status": "shutdown",
        "panes_closed": panes_closed
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

    let teammate_names: Vec<String> = team
        .agents
        .iter()
        .filter(|a| a.role == console_team::TeamAgentRole::Teammate)
        .map(|a| a.name.clone())
        .collect();
    let panes_closed = close_tmux_panes_for_agent_names(&teammate_names).unwrap_or(0);

    team_state.cleanup().await.map_err(team_err)?;

    json_output(&serde_json::json!({
        "team": team.team,
        "status": "cleaned_up",
        "agents_shutdown": 0,
        "panes_closed": panes_closed
    }))
}
