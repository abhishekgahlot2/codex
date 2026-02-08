use std::path::Path;
use std::sync::Arc;

use crate::AuthManager;
use crate::RolloutRecorder;
use crate::agent::AgentControl;
use crate::analytics_client::AnalyticsEventsClient;
use crate::client::ModelClient;
use crate::exec_policy::ExecPolicyManager;
use crate::file_watcher::FileWatcher;
use crate::hooks::Hooks;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::models_manager::manager::ModelsManager;
use crate::skills::SkillsManager;
use crate::state_db::StateDbHandle;
use crate::tools::sandboxing::ApprovalStore;
use crate::unified_exec::UnifiedExecProcessManager;
use codex_otel::OtelManager;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[allow(dead_code)]
pub(crate) struct ConsoleRuntimeServices {
    pub(crate) provider_registry: Arc<console_provider::ModelRegistry>,
    pub(crate) token_cost_calculator: Arc<console_provider::TokenCostCalculator>,
    pub(crate) guardrails: Mutex<console_runtime::GuardrailSet>,
    pub(crate) loop_state: Mutex<console_runtime::LoopState>,
    pub(crate) mode_policies: Vec<console_runtime::ModePolicy>,
    pub(crate) session_store: Arc<console_persist::JsonFileStore>,
    pub(crate) checkpoints: Mutex<console_persist::CheckpointManager>,
    pub(crate) plugin_registry: Mutex<console_plugin::PluginRegistry>,
    pub(crate) permission_policy: RwLock<console_security::PermissionPolicy>,
    pub(crate) audit_log: Mutex<console_security::AuditLog>,
}

impl ConsoleRuntimeServices {
    pub(crate) fn new(codex_home: &Path) -> Self {
        let provider_registry = Arc::new(console_provider::default_registry());
        let token_cost_calculator = Arc::new(console_provider::TokenCostCalculator::new(
            provider_registry.as_ref(),
        ));
        let session_store = Arc::new(console_persist::JsonFileStore::new(
            codex_home.join("console/sessions"),
        ));
        Self {
            provider_registry,
            token_cost_calculator,
            guardrails: Mutex::new(console_runtime::GuardrailSet::default()),
            loop_state: Mutex::new(console_runtime::LoopState::default()),
            mode_policies: console_runtime::default_mode_policies(),
            session_store,
            checkpoints: Mutex::new(console_persist::CheckpointManager::new()),
            plugin_registry: Mutex::new(console_plugin::PluginRegistry::new()),
            permission_policy: RwLock::new(console_security::PermissionPolicy::new(
                console_security::PermissionMode::Default,
            )),
            audit_log: Mutex::new(console_security::AuditLog::new(10_000)),
        }
    }
}

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: Arc<RwLock<McpConnectionManager>>,
    pub(crate) mcp_startup_cancellation_token: Mutex<CancellationToken>,
    pub(crate) unified_exec_manager: UnifiedExecProcessManager,
    pub(crate) analytics_events_client: AnalyticsEventsClient,
    pub(crate) hooks: Hooks,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) exec_policy: ExecPolicyManager,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) otel_manager: OtelManager,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) file_watcher: Arc<FileWatcher>,
    pub(crate) agent_control: AgentControl,
    #[allow(dead_code)]
    pub(crate) console: ConsoleRuntimeServices,
    // --- ConsoleAI team: team orchestration state ---
    pub(crate) team_state: Arc<console_team::TeamState>,
    pub(crate) state_db: Option<StateDbHandle>,
    /// Session-scoped model client shared across turns.
    pub(crate) model_client: ModelClient,
}
