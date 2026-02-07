//! UX parity tests: verify that Console v2 TUI configuration meets docs-derived requirements.

#[cfg(test)]
mod tests {
    use crate::density::{density_config, ConversationDensity};
    use crate::keymap::{default_keymap, KeyAction, KeyCombo};
    use crate::statusline::StatuslineData;
    use crate::theme::{default_theme, ThemeToken};

    // --- Composer UX Checklist ---

    #[test]
    fn ux_composer_has_blue_prompt_token() {
        let theme = default_theme();
        let prompt_color = theme.get(ThemeToken::Prompt);
        // Prompt should be blue (matching the blue chevron from Wave 0 fix)
        assert!(
            prompt_color.0.contains("58a6ff")
                || prompt_color.0.to_lowercase().contains("blue"),
            "prompt color should be blue, got: {}",
            prompt_color.0
        );
    }

    #[test]
    fn ux_composer_enter_submits_shift_enter_newline() {
        let keymap = default_keymap();
        let enter_action = keymap.action_for(&KeyCombo::key("Enter"));
        assert_eq!(enter_action, Some(&KeyAction::Submit), "Enter must submit");

        let shift_enter = keymap.action_for(&KeyCombo::shift("Enter"));
        assert_eq!(
            shift_enter,
            Some(&KeyAction::Newline),
            "Shift+Enter must insert newline"
        );
    }

    // --- Shortcut Surface Checklist ---

    #[test]
    fn ux_no_duplicate_shortcut_keys() {
        let keymap = default_keymap();
        let combos: Vec<_> = keymap.bindings.iter().map(|b| &b.combo).collect();
        for (i, a) in combos.iter().enumerate() {
            for (j, b) in combos.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "duplicate key binding: {:?}", a);
                }
            }
        }
    }

    #[test]
    fn ux_critical_shortcuts_present() {
        let keymap = default_keymap();
        // Per TASKS.md: ?, task toggle, model switch, multiline entry
        let actions: Vec<_> = keymap.bindings.iter().map(|b| &b.action).collect();
        assert!(
            actions.contains(&&KeyAction::ShowHelp),
            "? help shortcut missing"
        );
        assert!(
            actions.contains(&&KeyAction::ToggleTasks),
            "task toggle shortcut missing"
        );
        assert!(
            actions.contains(&&KeyAction::SwitchModel),
            "model switch shortcut missing"
        );
        assert!(
            actions.contains(&&KeyAction::Newline),
            "multiline entry shortcut missing"
        );
    }

    // --- Theme Checklist ---

    #[test]
    fn ux_theme_is_blue_black() {
        let theme = default_theme();
        assert_eq!(theme.name, "blue-black");
        // Background should be dark
        assert!(theme.bg.0.starts_with('#'), "bg should be a hex color");
        // Accent should be blue-ish
        assert!(
            theme.accent.0.contains("58a6ff") || theme.accent.0.contains("blue"),
            "accent should be blue"
        );
    }

    #[test]
    fn ux_theme_all_tokens_populated() {
        let theme = default_theme();
        let tokens = [
            ThemeToken::Bg,
            ThemeToken::Fg,
            ThemeToken::Accent,
            ThemeToken::AccentSecondary,
            ThemeToken::Muted,
            ThemeToken::Border,
            ThemeToken::Error,
            ThemeToken::Success,
            ThemeToken::Warning,
            ThemeToken::Prompt,
            ThemeToken::UserMsgBg,
            ThemeToken::AssistantMsgBg,
            ThemeToken::ToolResultBg,
            ThemeToken::StatuslineBg,
            ThemeToken::StatuslineFg,
        ];
        for token in &tokens {
            let color = theme.get(*token);
            assert!(!color.0.is_empty(), "theme token {:?} has empty color", token);
        }
    }

    // --- Statusline Checklist ---

    #[test]
    fn ux_statusline_segments_ordered_by_priority() {
        let mut data = StatuslineData::default();
        data.model = Some("Claude Opus 4.6".into());
        data.mode = Some("build".into());
        data.cost = Some("$0.42".into());
        data.total_tokens = Some(15000);

        let segments = data.to_segments();
        // Verify segments are ordered by priority (ascending)
        for window in segments.windows(2) {
            assert!(
                window[0].priority <= window[1].priority,
                "segments not ordered: {} (p{}) before {} (p{})",
                window[0].label,
                window[0].priority,
                window[1].label,
                window[1].priority
            );
        }
    }

    #[test]
    fn ux_statusline_model_is_first_segment() {
        let mut data = StatuslineData::default();
        data.model = Some("Claude Opus 4.6".into());
        data.mode = Some("build".into());

        let segments = data.to_segments();
        assert!(!segments.is_empty());
        assert_eq!(
            segments[0].label, "Model",
            "model should be the first statusline segment"
        );
    }

    // --- Density Checklist ---

    #[test]
    fn ux_compact_density_minimal_spacing() {
        let config = density_config(ConversationDensity::Compact);
        assert_eq!(config.message_gap, 0, "compact should have no message gap");
        assert!(
            !config.show_separators,
            "compact should hide separators"
        );
        assert!(
            config.collapse_tool_results,
            "compact should collapse tool results"
        );
    }

    #[test]
    fn ux_normal_density_balanced() {
        let config = density_config(ConversationDensity::Normal);
        assert_eq!(config.message_gap, 1, "normal should have 1-line gap");
        assert!(config.show_separators, "normal should show separators");
    }

    // --- Home State vs Active State ---

    #[test]
    fn ux_home_state_has_no_model_in_statusline() {
        // Home state should show minimal statusline (no model selected yet)
        let data = StatuslineData::default();
        let segments = data.to_segments();
        assert!(segments.is_empty(), "home state should have empty statusline");
    }

    #[test]
    fn ux_active_state_shows_model_and_mode() {
        let mut data = StatuslineData::default();
        data.model = Some("Claude Opus 4.6".into());
        data.mode = Some("build".into());

        let segments = data.to_segments();
        let labels: Vec<_> = segments.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"Model"), "active state must show model");
        assert!(labels.contains(&"Mode"), "active state must show mode");
    }
}
