use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
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
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn inactive_style(&self) -> Style {
        Style::default().fg(self.secondary)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.secondary)
    }

    pub fn focused_border_style(&self) -> Style {
        Style::default().fg(self.primary)
    }
}
