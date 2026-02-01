use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub primary: Color,
    pub secondary: Color,
    pub error: Color,
    pub foreground: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            secondary: Color::Gray,
            error: Color::Red,
            foreground: Color::White,
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
    pub const fn normal_style(&self) -> Style {
        Style::new().fg(self.foreground)
    }

    #[must_use]
    pub const fn inactive_style(&self) -> Style {
        Style::new().fg(self.secondary)
    }

    #[must_use]
    pub const fn border_style(&self) -> Style {
        Style::new().fg(self.secondary)
    }

    #[must_use]
    pub const fn focused_border_style(&self) -> Style {
        Style::new().fg(self.primary)
    }
}
