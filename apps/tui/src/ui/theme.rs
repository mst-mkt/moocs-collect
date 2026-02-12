use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub error: Color,
    pub success: Color,
    pub foreground: Color,
    pub dim: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Gray,
            error: Color::Red,
            success: Color::Green,
            foreground: Color::White,
            dim: Color::DarkGray,
        }
    }
}

impl Theme {
    #[must_use]
    pub const fn title_style(&self) -> Style {
        Style::new().fg(self.primary).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub const fn error_style(&self) -> Style {
        Style::new().fg(self.error).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub const fn success_style(&self) -> Style {
        Style::new().fg(self.success).add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub const fn normal_style(&self) -> Style {
        Style::new().fg(self.foreground)
    }

    #[must_use]
    pub const fn inactive_style(&self) -> Style {
        Style::new().fg(self.secondary)
    }

    #[must_use]
    pub const fn focused_border_style(&self) -> Style {
        Style::new().fg(self.primary)
    }

    #[must_use]
    pub const fn dim_style(&self) -> Style {
        Style::new().fg(self.dim)
    }

    #[must_use]
    pub const fn key_style(&self) -> Style {
        Style::new().fg(self.primary).add_modifier(Modifier::BOLD)
    }
}
