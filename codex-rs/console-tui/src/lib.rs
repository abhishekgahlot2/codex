pub mod density;
pub mod keymap;
pub mod statusline;
pub mod theme;
pub mod ux_parity;

pub use density::{density_config, ConversationDensity, DensityConfig};
pub use keymap::{default_keymap, KeyAction, KeyBinding, KeyCombo, Keymap};
pub use statusline::{StatuslineData, StatuslineSegment};
pub use theme::{default_theme, Color, Theme, ThemeToken};

pub mod badge;
pub use badge::{agent_badge_ansi, agent_color_code, agent_env_vars, pane_header_shell_cmd};

pub mod task_view;
pub use task_view::{format_agent_tree, format_task_checklist, TaskDisplayItem, TaskDisplayStatus};
