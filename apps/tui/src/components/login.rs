use collect::Credentials;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tui_input::{backend::crossterm::EventHandler, Input};

use super::Component;
use crate::ui::Theme;

#[derive(Debug, Clone)]
pub enum LoginAction {
    Submit(Credentials),
    SwitchField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputField {
    #[default]
    Username,
    Password,
}

pub struct LoginComponent {
    username_input: Input,
    password_input: Input,
    current_field: InputField,
    theme: Theme,
    loading: bool,
}

impl Component for LoginComponent {
    type Action = LoginAction;

    fn new() -> Self {
        Self {
            username_input: Input::default(),
            password_input: Input::default(),
            current_field: InputField::Username,
            theme: Theme::default(),
            loading: false,
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn update(&mut self, action: Self::Action) -> Option<Self::Action> {
        match action {
            LoginAction::SwitchField => {
                self.current_field = match self.current_field {
                    InputField::Username => InputField::Password,
                    InputField::Password => InputField::Username,
                };
                None
            }
            LoginAction::Submit(credentials) => {
                self.password_input.reset();
                Some(LoginAction::Submit(credentials))
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let [_, center, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(11),
            Constraint::Fill(1),
        ])
        .areas(area);

        let [_, form_area, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Max(50),
            Constraint::Fill(1),
        ])
        .areas(center);

        let [title_area, username_area, password_area, help_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .areas(form_area);

        // Title
        let title = Paragraph::new("MOOCs Collect")
            .alignment(Alignment::Center)
            .style(self.theme.title_style())
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(title, title_area);

        // Username
        let username_focused = self.current_field == InputField::Username;
        let username = Paragraph::new(self.username_input.value()).block(
            Block::default()
                .title(" Username ")
                .borders(Borders::ALL)
                .border_style(if username_focused {
                    self.theme.focused_border_style()
                } else {
                    self.theme.border_style()
                }),
        );
        frame.render_widget(username, username_area);

        // Password
        let password_focused = self.current_field == InputField::Password;
        let password_display = "*".repeat(self.password_input.value().len());
        let password = Paragraph::new(password_display).block(
            Block::default()
                .title(" Password ")
                .borders(Borders::ALL)
                .border_style(if password_focused {
                    self.theme.focused_border_style()
                } else {
                    self.theme.border_style()
                }),
        );
        frame.render_widget(password, password_area);

        // Help
        let help = Paragraph::new("Tab: Switch | Enter: Login | Ctrl+C: Quit")
            .alignment(Alignment::Center)
            .style(self.theme.inactive_style());
        frame.render_widget(help, help_area);

        // Cursor
        #[allow(clippy::cast_possible_truncation)]
        let (cursor_x, cursor_y) = if username_focused {
            (
                username_area.x + self.username_input.visual_cursor() as u16 + 1,
                username_area.y + 1,
            )
        } else {
            (
                password_area.x + self.password_input.visual_cursor() as u16 + 1,
                password_area.y + 1,
            )
        };
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

impl LoginComponent {
    pub const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<LoginAction> {
        match key.code {
            KeyCode::Tab => Some(LoginAction::SwitchField),
            KeyCode::Enter => {
                let username = self.username_input.value().trim().to_string();
                let password = self.password_input.value().to_string();
                if !username.is_empty() && !password.is_empty() {
                    Some(LoginAction::Submit(Credentials { username, password }))
                } else {
                    None
                }
            }
            _ => {
                match self.current_field {
                    InputField::Username => {
                        self.username_input.handle_event(&Event::Key(key));
                    }
                    InputField::Password => {
                        self.password_input.handle_event(&Event::Key(key));
                    }
                }
                None
            }
        }
    }
}
