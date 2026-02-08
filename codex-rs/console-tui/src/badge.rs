/// Agent badge rendering for Claude Code-style colored agent labels in tmux.
///
/// Provides ANSI-colored badges, pane header commands, and environment
/// variables for teammate processes so that each agent gets a visually
/// distinct identity in a tmux session.

/// Palette of ANSI color codes used for agent badges.
///
/// The order is chosen to maximise visual contrast between adjacent agents.
const PALETTE: &[u8] = &[
    32, // green
    33, // yellow
    36, // cyan
    35, // magenta
    34, // blue
    91, // bright_red
    92, // bright_green
    93, // bright_yellow
    94, // bright_blue
    95, // bright_magenta
];

/// Returns the ANSI color code for the agent at `index`, cycling through
/// [`PALETTE`] when the index exceeds the palette length.
pub fn agent_color_code(index: usize) -> u8 {
    PALETTE[index % PALETTE.len()]
}

/// Returns an ANSI-escaped bold colored badge string: `@name`.
///
/// Example output (for index 0, name "researcher"):
/// ```text
/// \x1b[1;32m@researcher\x1b[0m
/// ```
pub fn agent_badge_ansi(name: &str, index: usize) -> String {
    let color = agent_color_code(index);
    format!("\x1b[1;{color}m@{name}\x1b[0m")
}

/// Returns a shell command string that prints a colored header bar.
///
/// When executed via `tmux send-keys`, this prints a full-width separator
/// line with the agent name before the actual codex command starts.
pub fn pane_header_shell_cmd(name: &str, index: usize) -> String {
    let color = agent_color_code(index);
    format!(
        "printf '\\033[1;{color}m\u{2501}\u{2501}\u{2501} @{name} \
         \u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\
         \u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\
         \u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\
         \u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\
         \u{2501}\u{2501}\u{2501}\\033[0m\\n'"
    )
}

/// Returns environment variables to set for a teammate process.
///
/// The returned pairs are:
/// - `CONSOLE_AGENT_NAME` -- the agent's display name
/// - `CONSOLE_TEAM_NAME`  -- the team this agent belongs to
/// - `CONSOLE_AGENT_COLOR` -- the ANSI color code (as a decimal string)
pub fn agent_env_vars(name: &str, team_name: &str, index: usize) -> Vec<(String, String)> {
    let color = agent_color_code(index);
    vec![
        ("CONSOLE_AGENT_NAME".to_string(), name.to_string()),
        ("CONSOLE_TEAM_NAME".to_string(), team_name.to_string()),
        ("CONSOLE_AGENT_COLOR".to_string(), color.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_cycles() {
        // First full pass should match the palette exactly.
        for (i, &expected) in PALETTE.iter().enumerate() {
            assert_eq!(agent_color_code(i), expected, "mismatch at index {i}");
        }
        // After the palette length it should wrap around.
        assert_eq!(agent_color_code(PALETTE.len()), PALETTE[0]);
        assert_eq!(agent_color_code(PALETTE.len() + 1), PALETTE[1]);
        assert_eq!(agent_color_code(PALETTE.len() * 3 + 2), PALETTE[2]);
    }

    #[test]
    fn test_badge_contains_name() {
        let badge = agent_badge_ansi("researcher", 0);
        assert!(badge.contains("@researcher"), "badge should contain @name");
        assert!(badge.contains("\x1b[1;"), "badge should contain ANSI bold prefix");
        assert!(badge.contains("\x1b[0m"), "badge should contain ANSI reset suffix");
    }

    #[test]
    fn test_pane_header_is_valid_shell() {
        let header = pane_header_shell_cmd("coder", 1);
        assert!(header.contains("printf"), "header should be a printf command");
        // Yellow (index 1) -> color code 33
        assert!(header.contains("33"), "header should contain the color code");
        assert!(header.contains("@coder"), "header should contain the agent name");
    }

    #[test]
    fn test_env_vars_complete() {
        let vars = agent_env_vars("writer", "docs-team", 4);
        let names: Vec<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"CONSOLE_AGENT_NAME"));
        assert!(names.contains(&"CONSOLE_TEAM_NAME"));
        assert!(names.contains(&"CONSOLE_AGENT_COLOR"));

        // Verify values.
        let lookup = |key: &str| -> String {
            vars.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        assert_eq!(lookup("CONSOLE_AGENT_NAME"), "writer");
        assert_eq!(lookup("CONSOLE_TEAM_NAME"), "docs-team");
        // Index 4 -> blue (34).
        assert_eq!(lookup("CONSOLE_AGENT_COLOR"), "34");
    }
}
