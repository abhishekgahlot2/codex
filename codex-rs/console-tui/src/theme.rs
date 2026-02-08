use serde::Deserialize;
use serde::Serialize;

/// A color value (hex string or named color).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color(pub String);

impl Color {
    pub fn hex(s: &str) -> Self {
        Self(s.to_string())
    }
    pub fn named(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Named color tokens for theming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name.
    pub name: String,
    /// Primary background color.
    pub bg: Color,
    /// Primary foreground (text) color.
    pub fg: Color,
    /// Accent color (highlights, active elements).
    pub accent: Color,
    /// Secondary accent (links, secondary highlights).
    pub accent_secondary: Color,
    /// Muted/dim text color.
    pub muted: Color,
    /// Border color.
    pub border: Color,
    /// Error/danger color.
    pub error: Color,
    /// Success color.
    pub success: Color,
    /// Warning color.
    pub warning: Color,
    /// Composer prompt (chevron) color.
    pub prompt: Color,
    /// User message background.
    pub user_msg_bg: Color,
    /// Assistant message background.
    pub assistant_msg_bg: Color,
    /// Tool result background.
    pub tool_result_bg: Color,
    /// Statusline background.
    pub statusline_bg: Color,
    /// Statusline foreground.
    pub statusline_fg: Color,
}

/// The default blue-black Console v2 theme.
pub fn default_theme() -> Theme {
    Theme {
        name: "blue-black".into(),
        bg: Color::hex("#0d1117"),
        fg: Color::hex("#c9d1d9"),
        accent: Color::hex("#58a6ff"),
        accent_secondary: Color::hex("#79c0ff"),
        muted: Color::hex("#484f58"),
        border: Color::hex("#30363d"),
        error: Color::hex("#f85149"),
        success: Color::hex("#3fb950"),
        warning: Color::hex("#d29922"),
        prompt: Color::hex("#58a6ff"),
        user_msg_bg: Color::hex("#161b22"),
        assistant_msg_bg: Color::hex("#0d1117"),
        tool_result_bg: Color::hex("#161b22"),
        statusline_bg: Color::hex("#161b22"),
        statusline_fg: Color::hex("#8b949e"),
    }
}

/// Token identifiers for programmatic theme access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeToken {
    Bg,
    Fg,
    Accent,
    AccentSecondary,
    Muted,
    Border,
    Error,
    Success,
    Warning,
    Prompt,
    UserMsgBg,
    AssistantMsgBg,
    ToolResultBg,
    StatuslineBg,
    StatuslineFg,
}

impl Theme {
    /// Look up a color by token.
    pub fn get(&self, token: ThemeToken) -> &Color {
        match token {
            ThemeToken::Bg => &self.bg,
            ThemeToken::Fg => &self.fg,
            ThemeToken::Accent => &self.accent,
            ThemeToken::AccentSecondary => &self.accent_secondary,
            ThemeToken::Muted => &self.muted,
            ThemeToken::Border => &self.border,
            ThemeToken::Error => &self.error,
            ThemeToken::Success => &self.success,
            ThemeToken::Warning => &self.warning,
            ThemeToken::Prompt => &self.prompt,
            ThemeToken::UserMsgBg => &self.user_msg_bg,
            ThemeToken::AssistantMsgBg => &self.assistant_msg_bg,
            ThemeToken::ToolResultBg => &self.tool_result_bg,
            ThemeToken::StatuslineBg => &self.statusline_bg,
            ThemeToken::StatuslineFg => &self.statusline_fg,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_values() {
        let theme = default_theme();
        assert_eq!(theme.name, "blue-black");
        assert_eq!(theme.bg, Color::hex("#0d1117"));
        assert_eq!(theme.fg, Color::hex("#c9d1d9"));
        assert_eq!(theme.accent, Color::hex("#58a6ff"));
        assert_eq!(theme.error, Color::hex("#f85149"));
        assert_eq!(theme.success, Color::hex("#3fb950"));
        assert_eq!(theme.warning, Color::hex("#d29922"));
    }

    #[test]
    fn token_lookup() {
        let theme = default_theme();
        assert_eq!(theme.get(ThemeToken::Bg), &Color::hex("#0d1117"));
        assert_eq!(theme.get(ThemeToken::Accent), &Color::hex("#58a6ff"));
        assert_eq!(theme.get(ThemeToken::StatuslineFg), &Color::hex("#8b949e"));
    }

    #[test]
    fn all_tokens_accessible() {
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
        for token in tokens {
            let color = theme.get(token);
            assert!(!color.0.is_empty(), "Token {token:?} returned empty color");
        }
    }

    #[test]
    fn serialization_roundtrip() {
        let theme = default_theme();
        let json = serde_json::to_string(&theme).unwrap();
        let deserialized: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, theme.name);
        assert_eq!(deserialized.bg, theme.bg);
        assert_eq!(deserialized.fg, theme.fg);
        assert_eq!(deserialized.accent, theme.accent);
    }

    #[test]
    fn color_constructors() {
        let hex = Color::hex("#ff0000");
        assert_eq!(hex.0, "#ff0000");

        let named = Color::named("red");
        assert_eq!(named.0, "red");
    }

    #[test]
    fn theme_token_serialization_roundtrip() {
        let token = ThemeToken::AccentSecondary;
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, "\"accent_secondary\"");
        let deserialized: ThemeToken = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, token);
    }
}
