use serde::{Deserialize, Serialize};

/// A key combination.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    /// Key name (e.g., "Enter", "?", "m", "t", "Esc").
    pub key: String,
    /// Modifier keys.
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyCombo {
    pub fn key(k: &str) -> Self {
        Self {
            key: k.into(),
            shift: false,
            ctrl: false,
            alt: false,
        }
    }
    pub fn shift(k: &str) -> Self {
        Self {
            key: k.into(),
            shift: true,
            ctrl: false,
            alt: false,
        }
    }
    pub fn ctrl(k: &str) -> Self {
        Self {
            key: k.into(),
            shift: false,
            ctrl: true,
            alt: false,
        }
    }
}

/// An action that can be bound to a key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    /// Submit the current input.
    Submit,
    /// Insert a newline (multiline mode).
    Newline,
    /// Show help / keyboard shortcut reference.
    ShowHelp,
    /// Toggle task panel visibility.
    ToggleTasks,
    /// Switch model.
    SwitchModel,
    /// Switch execution mode (build/plan/review).
    SwitchMode,
    /// Cancel current operation.
    Cancel,
    /// Navigate history up.
    HistoryUp,
    /// Navigate history down.
    HistoryDown,
    /// Clear screen.
    ClearScreen,
    /// Focus next teammate.
    FocusNext,
    /// Focus previous teammate.
    FocusPrev,
    /// Custom action (for plugins).
    Custom(String),
}

/// A single key binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub combo: KeyCombo,
    pub action: KeyAction,
    pub description: String,
}

/// Full keymap configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keymap {
    pub bindings: Vec<KeyBinding>,
}

impl Keymap {
    /// Find the action for a given key combo.
    pub fn action_for(&self, combo: &KeyCombo) -> Option<&KeyAction> {
        self.bindings
            .iter()
            .find(|b| b.combo == *combo)
            .map(|b| &b.action)
    }
}

/// Default keymap with critical controls.
pub fn default_keymap() -> Keymap {
    Keymap {
        bindings: vec![
            KeyBinding {
                combo: KeyCombo::key("Enter"),
                action: KeyAction::Submit,
                description: "Submit input".into(),
            },
            KeyBinding {
                combo: KeyCombo::shift("Enter"),
                action: KeyAction::Newline,
                description: "Insert newline".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("?"),
                action: KeyAction::ShowHelp,
                description: "Show keyboard shortcuts".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("t"),
                action: KeyAction::ToggleTasks,
                description: "Toggle task panel".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("m"),
                action: KeyAction::SwitchModel,
                description: "Switch model".into(),
            },
            KeyBinding {
                combo: KeyCombo::ctrl("m"),
                action: KeyAction::SwitchMode,
                description: "Switch execution mode".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("Esc"),
                action: KeyAction::Cancel,
                description: "Cancel current operation".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("Up"),
                action: KeyAction::HistoryUp,
                description: "Previous input history".into(),
            },
            KeyBinding {
                combo: KeyCombo::key("Down"),
                action: KeyAction::HistoryDown,
                description: "Next input history".into(),
            },
            KeyBinding {
                combo: KeyCombo::ctrl("l"),
                action: KeyAction::ClearScreen,
                description: "Clear screen".into(),
            },
            KeyBinding {
                combo: KeyCombo::ctrl("n"),
                action: KeyAction::FocusNext,
                description: "Focus next teammate".into(),
            },
            KeyBinding {
                combo: KeyCombo::ctrl("p"),
                action: KeyAction::FocusPrev,
                description: "Focus previous teammate".into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymap_has_12_bindings() {
        let km = default_keymap();
        assert_eq!(km.bindings.len(), 12);
    }

    #[test]
    fn action_for_lookup() {
        let km = default_keymap();
        assert_eq!(
            km.action_for(&KeyCombo::key("Enter")),
            Some(&KeyAction::Submit)
        );
        assert_eq!(
            km.action_for(&KeyCombo::shift("Enter")),
            Some(&KeyAction::Newline)
        );
        assert_eq!(
            km.action_for(&KeyCombo::ctrl("l")),
            Some(&KeyAction::ClearScreen)
        );
        assert!(km.action_for(&KeyCombo::key("z")).is_none());
    }

    #[test]
    fn critical_controls_present() {
        let km = default_keymap();
        // ? for help
        assert_eq!(
            km.action_for(&KeyCombo::key("?")),
            Some(&KeyAction::ShowHelp)
        );
        // t for tasks
        assert_eq!(
            km.action_for(&KeyCombo::key("t")),
            Some(&KeyAction::ToggleTasks)
        );
        // m for model
        assert_eq!(
            km.action_for(&KeyCombo::key("m")),
            Some(&KeyAction::SwitchModel)
        );
        // Esc for cancel
        assert_eq!(
            km.action_for(&KeyCombo::key("Esc")),
            Some(&KeyAction::Cancel)
        );
        // Enter for submit
        assert_eq!(
            km.action_for(&KeyCombo::key("Enter")),
            Some(&KeyAction::Submit)
        );
        // Shift+Enter for newline
        assert_eq!(
            km.action_for(&KeyCombo::shift("Enter")),
            Some(&KeyAction::Newline)
        );
    }

    #[test]
    fn serialization_roundtrip() {
        let km = default_keymap();
        let json = serde_json::to_string(&km).unwrap();
        let deserialized: Keymap = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.bindings.len(), km.bindings.len());
        // Verify first binding roundtrips correctly
        assert_eq!(deserialized.bindings[0].combo, km.bindings[0].combo);
        assert_eq!(deserialized.bindings[0].action, km.bindings[0].action);
    }

    #[test]
    fn key_combo_constructors() {
        let plain = KeyCombo::key("a");
        assert_eq!(plain.key, "a");
        assert!(!plain.shift);
        assert!(!plain.ctrl);
        assert!(!plain.alt);

        let shifted = KeyCombo::shift("Enter");
        assert!(shifted.shift);
        assert!(!shifted.ctrl);

        let ctrl = KeyCombo::ctrl("c");
        assert!(ctrl.ctrl);
        assert!(!ctrl.shift);
    }

    #[test]
    fn custom_action_serialization() {
        let action = KeyAction::Custom("my_plugin_action".into());
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: KeyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }
}
