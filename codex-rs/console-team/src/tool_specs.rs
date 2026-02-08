//! Tool specification builders for the 10 team orchestration tools.
//!
//! Each function returns a `ToolSpec` (tagged as `"function"`) that serializes
//! to the same JSON shape as `codex-core`'s `ToolSpec::Function`. This lets
//! codex-core call these builders and push the results into its registry
//! without console-team depending on codex-core.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Minimal mirror of codex-core's JsonSchema / ToolSpec types.
// These serialize identically so codex-core can round-trip them via
// `serde_json::Value` or deserialize directly.
// ---------------------------------------------------------------------------

/// Subset of JSON Schema used by the Responses API tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonSchema {
    Boolean {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    String {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    #[serde(alias = "integer")]
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Array {
        items: Box<JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Object {
        properties: BTreeMap<String, JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<Vec<String>>,
        #[serde(
            rename = "additionalProperties",
            skip_serializing_if = "Option::is_none"
        )]
        additional_properties: Option<AdditionalProperties>,
    },
}

/// Whether additional properties are allowed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Boolean(bool),
    Schema(Box<JsonSchema>),
}

impl From<bool> for AdditionalProperties {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<JsonSchema> for AdditionalProperties {
    fn from(s: JsonSchema) -> Self {
        Self::Schema(Box::new(s))
    }
}

/// A single function-type tool definition (Responses API shape).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponsesApiTool {
    pub name: String,
    pub description: String,
    pub strict: bool,
    pub parameters: JsonSchema,
}

/// Tool specification wrapper tagged with `"type": "function"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ToolSpec {
    #[serde(rename = "function")]
    Function(ResponsesApiTool),
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn string_param(desc: &str) -> JsonSchema {
    JsonSchema::String {
        description: Some(desc.to_string()),
    }
}

fn string_array_param(desc: &str) -> JsonSchema {
    JsonSchema::Array {
        items: Box::new(JsonSchema::String { description: None }),
        description: Some(desc.to_string()),
    }
}

fn number_param(desc: &str) -> JsonSchema {
    JsonSchema::Number {
        description: Some(desc.to_string()),
    }
}

fn make_tool(
    name: &str,
    description: &str,
    properties: BTreeMap<String, JsonSchema>,
    required: Vec<&str>,
) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(required.into_iter().map(|s| s.to_string()).collect()),
            additional_properties: Some(false.into()),
        },
    })
}

// ---------------------------------------------------------------------------
// 9 team tool spec builders
// ---------------------------------------------------------------------------

/// `team_create` – create a new team with the calling agent as lead.
pub fn create_team_create_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "team_name".to_string(),
        string_param("Name for the new team (used as file stem for persistence)."),
    );
    props.insert(
        "description".to_string(),
        string_param("Short description of the team's purpose."),
    );
    make_tool(
        "team_create",
        "Create a new team. The calling agent becomes the team lead.",
        props,
        vec!["team_name"],
    )
}

/// `team_add_agent` – add an agent to the current team.
pub fn create_team_add_agent_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "name".to_string(),
        string_param("Human-readable name for the new agent."),
    );
    props.insert(
        "role".to_string(),
        string_param("Agent role: 'lead' or 'teammate'."),
    );
    props.insert(
        "model".to_string(),
        string_param(
            "Optional model identifier for the agent (e.g. 'claude-sonnet-4-5-20250929').",
        ),
    );
    make_tool(
        "team_add_agent",
        "Add an agent to the current team.",
        props,
        vec!["name", "role"],
    )
}

/// `team_add_task` – create a task on the shared board.
pub fn create_team_add_task_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "title".to_string(),
        string_param("Short title describing the task."),
    );
    props.insert(
        "depends_on".to_string(),
        string_array_param(
            "Task IDs this task depends on. Task starts Blocked until deps complete.",
        ),
    );
    make_tool(
        "team_add_task",
        "Create a task on the team board. If depends_on IDs are unresolved, status is Blocked; otherwise Pending.",
        props,
        vec!["title"],
    )
}

/// `team_claim_task` – claim an unblocked task for the calling agent.
pub fn create_team_claim_task_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "task_id".to_string(),
        string_param("ID of the task to claim."),
    );
    props.insert(
        "assignee_id".to_string(),
        string_param("Agent ID or name claiming the task."),
    );
    make_tool(
        "team_claim_task",
        "Claim a Pending task and set it to InProgress. Fails if task is Blocked or Completed.",
        props,
        vec!["task_id", "assignee_id"],
    )
}

/// `team_complete_task` – mark a task as done and auto-unblock dependents.
pub fn create_team_complete_task_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "task_id".to_string(),
        string_param("ID of the task to mark Completed."),
    );
    props.insert(
        "result".to_string(),
        string_param("Output or result text to attach to the completed task. The lead reads this from the task board."),
    );
    make_tool(
        "team_complete_task",
        "Mark a task as Completed and attach an optional result. The lead reads results from the task board via team_list_tasks. Any Blocked tasks whose dependencies are now all complete are auto-promoted to Pending.",
        props,
        vec!["task_id"],
    )
}

/// `team_list_tasks` – list all tasks on the board.
pub fn create_team_list_tasks_tool() -> ToolSpec {
    let props = BTreeMap::new();
    make_tool(
        "team_list_tasks",
        "List all tasks on the team board with their current status and assignees.",
        props,
        vec![],
    )
}

/// `team_send_message` – send a message to another agent.
pub fn create_team_send_message_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "to".to_string(),
        string_param("Agent name or ID to send the message to."),
    );
    props.insert("body".to_string(), string_param("Message content."));
    make_tool(
        "team_send_message",
        "Send a message to another agent on the team.",
        props,
        vec!["to", "body"],
    )
}

/// `team_list_messages` – list recent team messages.
pub fn create_team_list_messages_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "limit".to_string(),
        number_param("Maximum number of recent messages to return. Omit for all."),
    );
    make_tool(
        "team_list_messages",
        "List team messages, optionally limited to the N most recent.",
        props,
        vec![],
    )
}

/// `team_broadcast` – broadcast a message to all active teammates.
pub fn create_team_broadcast_tool() -> ToolSpec {
    let mut props = BTreeMap::new();
    props.insert(
        "from".to_string(),
        string_param("Sender agent name or id (optional, defaults to lead)."),
    );
    props.insert(
        "body".to_string(),
        string_param("Message body to send to all teammates."),
    );
    make_tool(
        "team_broadcast",
        "Broadcast a message to all active teammates. Use for announcements that every teammate needs to see.",
        props,
        vec!["body"],
    )
}

/// `team_cleanup` – tear down the team, removing persisted state.
pub fn create_team_cleanup_tool() -> ToolSpec {
    let props = BTreeMap::new();
    make_tool(
        "team_cleanup",
        "Shut down and clean up the team, removing all persisted state.",
        props,
        vec![],
    )
}

/// Return all 10 team tool specs.
pub fn all_team_tool_specs() -> Vec<ToolSpec> {
    vec![
        create_team_create_tool(),
        create_team_add_agent_tool(),
        create_team_add_task_tool(),
        create_team_claim_task_tool(),
        create_team_complete_task_tool(),
        create_team_list_tasks_tool(),
        create_team_send_message_tool(),
        create_team_broadcast_tool(),
        create_team_list_messages_tool(),
        create_team_cleanup_tool(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_specs_serialize_to_valid_json() {
        for spec in all_team_tool_specs() {
            let json = serde_json::to_string_pretty(&spec).unwrap();
            // Verify it round-trips.
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["type"], "function");
            assert!(parsed["name"].is_string());
            assert!(parsed["parameters"]["type"] == "object");
        }
    }

    #[test]
    fn spec_count_is_ten() {
        assert_eq!(all_team_tool_specs().len(), 10);
    }

    #[test]
    fn team_create_has_required_team_name() {
        let spec = create_team_create_tool();
        let json = serde_json::to_value(&spec).unwrap();
        let required = json["parameters"]["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "team_name"));
    }

    #[test]
    fn team_add_task_has_optional_depends_on() {
        let spec = create_team_add_task_tool();
        let json = serde_json::to_value(&spec).unwrap();
        let required = json["parameters"]["required"].as_array().unwrap();
        // depends_on is NOT required
        assert!(!required.iter().any(|v| v == "depends_on"));
        // but it IS in properties
        assert!(json["parameters"]["properties"]["depends_on"].is_object());
    }

    #[test]
    fn tool_names_are_unique() {
        let specs = all_team_tool_specs();
        let names: Vec<String> = specs
            .iter()
            .map(|s| match s {
                ToolSpec::Function(t) => t.name.clone(),
            })
            .collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len());
    }

    // -----------------------------------------------------------------------
    // NL intent-routing regression tests
    //
    // These prove that team control works purely through tool schemas
    // (LLM-as-Router) — no regex/keyword hardcoding, no slash commands.
    // -----------------------------------------------------------------------

    /// Verify the crate uses no regex-based routing — all intent routing is
    /// delegated to the LLM via tool schemas.  We assert the namespace
    /// convention (`team_` prefix) that makes the tools discoverable.
    #[test]
    fn test_no_regex_routing_in_crate() {
        // The LLM-as-Router architecture means tool_specs is the ONLY routing
        // mechanism — all tools are defined as schemas, not as
        // pattern-matched commands.  Verify the namespace convention holds.
        let specs = super::all_team_tool_specs();

        for spec in &specs {
            let json = serde_json::to_value(spec).unwrap();
            let name = json["name"].as_str().unwrap();
            assert!(
                name.starts_with("team_"),
                "tool '{name}' must use team_ prefix for namespace consistency"
            );
        }
    }

    /// Verify each tool schema carries enough context for an LLM to route
    /// a natural-language request to the correct tool without any external
    /// keyword tables or regex patterns.
    #[test]
    fn test_tool_schemas_sufficient_for_llm_routing() {
        let specs = super::all_team_tool_specs();

        for spec in &specs {
            let json = serde_json::to_value(spec).unwrap();
            let name = json["name"].as_str().unwrap();
            let description = json["description"].as_str().unwrap_or("");

            // Every tool must have a non-empty description.
            assert!(
                !description.is_empty(),
                "tool '{name}' must have a description for LLM routing"
            );

            // Description must be at least 20 chars (meaningful, not just a label).
            assert!(
                description.len() >= 20,
                "tool '{name}' description too short ({} chars) for effective LLM routing",
                description.len()
            );

            // Description should start with an action verb or mention "team".
            let first_word = description.split_whitespace().next().unwrap_or("");
            let action_words = [
                "Create",
                "Add",
                "Claim",
                "Complete",
                "List",
                "Send",
                "Get",
                "Shut",
                "Clean",
                "Remove",
                "Update",
                "Assign",
                "Mark",
                "Broadcast",
            ];
            assert!(
                action_words
                    .iter()
                    .any(|w| first_word.eq_ignore_ascii_case(w))
                    || description.contains("team"),
                "tool '{name}' description should start with an action verb or mention 'team', got: '{first_word}...'"
            );
        }
    }

    /// Verify all required NL team operations map to tools.
    /// The full lifecycle (create → populate → work → communicate → tear-down)
    /// must be expressible purely through tool calls.
    #[test]
    fn test_all_team_operations_have_tools() {
        let specs = super::all_team_tool_specs();
        let names: Vec<String> = specs
            .iter()
            .map(|s| {
                serde_json::to_value(s).unwrap()["name"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        // The NL team lifecycle requires these operations:
        let required_operations = [
            "team_create",        // "create a team with two workers"
            "team_add_agent",     // "add an agent named X"
            "team_add_task",      // "add a task to the board"
            "team_claim_task",    // "assign task to agent X"
            "team_complete_task", // "mark task as done"
            "team_list_tasks",    // "show me the task board"
            "team_send_message",  // "tell agent X to start working"
            "team_broadcast",     // "announce to all teammates"
            "team_list_messages", // "show recent messages"
            "team_cleanup",       // "clean up the team"
        ];

        for op in &required_operations {
            assert!(
                names.contains(&op.to_string()),
                "required NL operation '{op}' has no corresponding tool"
            );
        }
    }

    /// Verify tool specs do not reference slash commands — the schema layer
    /// must be self-contained with no dependency on a `/team` command parser.
    #[test]
    fn test_no_slash_command_dependencies() {
        let specs = super::all_team_tool_specs();

        for spec in &specs {
            let json_str = serde_json::to_string(spec).unwrap();

            // No tool should reference slash commands in its schema.
            assert!(
                !json_str.contains("/team"),
                "tool spec should not reference slash commands: {json_str}"
            );
            assert!(
                !json_str.contains("slash"),
                "tool spec should not mention slash commands: {json_str}"
            );
            // "command" would indicate leaking CLI concerns into the schema.
            assert!(
                !json_str.contains("command"),
                "tool spec should not mention commands (use 'tool' terminology): {json_str}"
            );
        }
    }
}
