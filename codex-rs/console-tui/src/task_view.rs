use serde::{Deserialize, Serialize};

/// Visual status for a task in the checklist display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskDisplayStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

/// A single task item for rendering in a checklist UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDisplayItem {
    pub title: String,
    pub status: TaskDisplayStatus,
    pub assignee: Option<String>,
}

/// Format a task list as an ANSI-colored checklist string.
///
/// Output like:
/// ```text
///   ✓ Write haiku about nature (poet-nature)    [strikethrough, green]
///   ■ Write haiku about tech (poet-tech)         [bold, yellow]
///   □ Write haiku about time                     [normal]
///   ⊘ Blocked task                               [dim]
/// ```
pub fn format_task_checklist(tasks: &[TaskDisplayItem]) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::with_capacity(tasks.len());

    for task in tasks {
        let assignee_suffix = match &task.assignee {
            Some(name) => format!(" ({})", name),
            None => String::new(),
        };

        let line = match task.status {
            TaskDisplayStatus::Completed => {
                // Green (\x1b[32m) + strikethrough (\x1b[9m), assignee before reset
                format!(
                    "\x1b[32m\x1b[9m  \u{2713} {}{}\x1b[0m",
                    task.title, assignee_suffix
                )
            }
            TaskDisplayStatus::InProgress => {
                // Bold yellow (\x1b[1;33m), assignee before reset
                format!(
                    "\x1b[1;33m  \u{25a0} {}{}\x1b[0m",
                    task.title, assignee_suffix
                )
            }
            TaskDisplayStatus::Pending => {
                // Plain text, no ANSI codes
                format!("  \u{25a1} {}{}", task.title, assignee_suffix)
            }
            TaskDisplayStatus::Blocked => {
                // Dim (\x1b[2m), assignee before reset
                format!(
                    "\x1b[2m  \u{2298} {}{}\x1b[0m",
                    task.title, assignee_suffix
                )
            }
        };

        lines.push(line);
    }

    lines.join("\n")
}

/// Format a compact agent tree view like Claude Code's:
///
/// ```text
/// 3 agents launched (ctrl+o to expand)
///   ├── @poet-nature
///   │   └── Write haiku about nature
///   ├── @poet-tech
///   │   └── Write haiku about tech
///   └── @poet-time
///       └── Write haiku about time
/// ```
///
/// `agents` is a slice of `(name, optional_task_description)` tuples.
pub fn format_agent_tree(agents: &[(String, Option<String>)]) -> String {
    if agents.is_empty() {
        return String::new();
    }

    let count = agents.len();
    let mut lines: Vec<String> = Vec::with_capacity(1 + count * 2);

    // Header line
    lines.push(format!("{} agents launched", count));

    for (i, (name, task)) in agents.iter().enumerate() {
        let is_last = i == count - 1;
        let branch = if is_last { "\u{2514}\u{2500}\u{2500}" } else { "\u{251c}\u{2500}\u{2500}" };
        let continuation = if is_last { "    " } else { "\u{2502}   " };

        // Agent name line
        lines.push(format!("  {} @{}", branch, name));

        // Optional task description as child node
        if let Some(desc) = task {
            lines.push(format!("  {} \u{2514}\u{2500}\u{2500} {}", continuation, desc));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checklist_all_statuses() {
        let tasks = vec![
            TaskDisplayItem {
                title: "Completed task".to_string(),
                status: TaskDisplayStatus::Completed,
                assignee: None,
            },
            TaskDisplayItem {
                title: "In progress task".to_string(),
                status: TaskDisplayStatus::InProgress,
                assignee: None,
            },
            TaskDisplayItem {
                title: "Pending task".to_string(),
                status: TaskDisplayStatus::Pending,
                assignee: None,
            },
            TaskDisplayItem {
                title: "Blocked task".to_string(),
                status: TaskDisplayStatus::Blocked,
                assignee: None,
            },
        ];

        let output = format_task_checklist(&tasks);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 4);

        // Completed: green + strikethrough + checkmark
        assert!(lines[0].contains('\u{2713}'), "Completed should have checkmark");
        assert!(lines[0].contains("\x1b[32m"), "Completed should be green");
        assert!(lines[0].contains("\x1b[9m"), "Completed should be strikethrough");
        assert!(lines[0].contains("Completed task"));

        // InProgress: bold yellow + filled square
        assert!(lines[1].contains('\u{25a0}'), "InProgress should have filled square");
        assert!(lines[1].contains("\x1b[1;33m"), "InProgress should be bold yellow");
        assert!(lines[1].contains("In progress task"));

        // Pending: plain + empty square
        assert!(lines[2].contains('\u{25a1}'), "Pending should have empty square");
        assert!(!lines[2].contains("\x1b["), "Pending should have no ANSI codes");
        assert!(lines[2].contains("Pending task"));

        // Blocked: dim + circled slash
        assert!(lines[3].contains('\u{2298}'), "Blocked should have circled slash");
        assert!(lines[3].contains("\x1b[2m"), "Blocked should be dim");
        assert!(lines[3].contains("Blocked task"));
    }

    #[test]
    fn test_checklist_with_assignees() {
        let tasks = vec![
            TaskDisplayItem {
                title: "Write haiku about nature".to_string(),
                status: TaskDisplayStatus::Completed,
                assignee: Some("poet-nature".to_string()),
            },
            TaskDisplayItem {
                title: "Write haiku about tech".to_string(),
                status: TaskDisplayStatus::InProgress,
                assignee: Some("poet-tech".to_string()),
            },
            TaskDisplayItem {
                title: "Write haiku about time".to_string(),
                status: TaskDisplayStatus::Pending,
                assignee: Some("poet-time".to_string()),
            },
            TaskDisplayItem {
                title: "Blocked by dependency".to_string(),
                status: TaskDisplayStatus::Blocked,
                assignee: Some("blocked-agent".to_string()),
            },
        ];

        let output = format_task_checklist(&tasks);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 4);

        // Verify assignee names appear in each line
        assert!(lines[0].contains("(poet-nature)"), "Completed should show assignee");
        assert!(lines[1].contains("(poet-tech)"), "InProgress should show assignee");
        assert!(lines[2].contains("(poet-time)"), "Pending should show assignee");
        assert!(lines[3].contains("(blocked-agent)"), "Blocked should show assignee");

        // Verify assignee is before the ANSI reset for styled lines
        for (i, line) in lines.iter().enumerate() {
            if i == 2 {
                // Pending has no ANSI codes, skip
                continue;
            }
            let reset_pos = line.rfind("\x1b[0m").expect("styled line should have reset");
            let assignee_pos = line.find('(').expect("line should have assignee");
            assert!(
                assignee_pos < reset_pos,
                "assignee should appear before ANSI reset on line {}",
                i
            );
        }
    }

    #[test]
    fn test_agent_tree_formatting() {
        let agents = vec![
            ("poet-nature".to_string(), Some("Write haiku about nature".to_string())),
            ("poet-tech".to_string(), Some("Write haiku about tech".to_string())),
            ("poet-time".to_string(), Some("Write haiku about time".to_string())),
        ];

        let output = format_agent_tree(&agents);
        let lines: Vec<&str> = output.lines().collect();

        // Header + 3 agents * 2 lines each (name + task) = 7 lines
        assert_eq!(lines.len(), 7);

        // Header
        assert_eq!(lines[0], "3 agents launched");

        // First agent (not last): uses ├──
        assert!(lines[1].contains("\u{251c}\u{2500}\u{2500}"), "non-last should use ├──");
        assert!(lines[1].contains("@poet-nature"));
        assert!(lines[2].contains("Write haiku about nature"));
        assert!(lines[2].contains("\u{2502}"), "non-last child should have │ continuation");

        // Second agent (not last): uses ├──
        assert!(lines[3].contains("\u{251c}\u{2500}\u{2500}"), "non-last should use ├──");
        assert!(lines[3].contains("@poet-tech"));
        assert!(lines[4].contains("Write haiku about tech"));

        // Third agent (last): uses └──
        assert!(lines[5].contains("\u{2514}\u{2500}\u{2500}"), "last should use └──");
        assert!(lines[5].contains("@poet-time"));
        assert!(lines[6].contains("Write haiku about time"));
        // Last agent's child should NOT have │ continuation
        assert!(!lines[6].contains('\u{2502}'), "last child should not have │ continuation");
    }

    #[test]
    fn test_agent_tree_without_tasks() {
        let agents = vec![
            ("worker-1".to_string(), None),
            ("worker-2".to_string(), Some("Has a task".to_string())),
            ("worker-3".to_string(), None),
        ];

        let output = format_agent_tree(&agents);
        let lines: Vec<&str> = output.lines().collect();

        // Header + agent1 (no task, 1 line) + agent2 (with task, 2 lines) + agent3 (no task, 1 line) = 5
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "3 agents launched");
        assert!(lines[1].contains("@worker-1"));
        assert!(lines[2].contains("@worker-2"));
        assert!(lines[3].contains("Has a task"));
        assert!(lines[4].contains("@worker-3"));
    }

    #[test]
    fn test_empty_inputs() {
        assert_eq!(format_task_checklist(&[]), "");
        assert_eq!(format_agent_tree(&[]), "");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let item = TaskDisplayItem {
            title: "Test task".to_string(),
            status: TaskDisplayStatus::InProgress,
            assignee: Some("agent-1".to_string()),
        };

        let json = serde_json::to_string(&item).expect("serialize");
        let deserialized: TaskDisplayItem = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.title, "Test task");
        assert_eq!(deserialized.status, TaskDisplayStatus::InProgress);
        assert_eq!(deserialized.assignee, Some("agent-1".to_string()));
    }
}
