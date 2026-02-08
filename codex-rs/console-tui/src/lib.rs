pub mod density;
pub mod keymap;
pub mod statusline;
pub mod theme;
pub mod ux_parity;

pub use density::ConversationDensity;
pub use density::DensityConfig;
pub use density::density_config;
pub use keymap::KeyAction;
pub use keymap::KeyBinding;
pub use keymap::KeyCombo;
pub use keymap::Keymap;
pub use keymap::default_keymap;
pub use statusline::StatuslineData;
pub use statusline::StatuslineSegment;
pub use theme::Color;
pub use theme::Theme;
pub use theme::ThemeToken;
pub use theme::default_theme;

pub mod badge;
pub use badge::agent_badge_ansi;
pub use badge::agent_color_code;
pub use badge::agent_env_vars;
pub use badge::pane_header_shell_cmd;

pub mod task_view;
pub use task_view::TaskDisplayItem;
pub use task_view::TaskDisplayStatus;
pub use task_view::format_agent_tree;
pub use task_view::format_task_checklist;
