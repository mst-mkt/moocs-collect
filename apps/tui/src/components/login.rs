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
}

impl Component for LoginComponent {
    type Action = LoginAction;

    fn new() -> Self {
        Self {
            username_input: Input::default(),
            password_input: Input::default(),
            current_field: InputField::default(),
            theme: Theme::default(),
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let areas = compute_form_areas(area);

        // Title
        let title = Paragraph::new("MOOCs Collect")
            .alignment(Alignment::Center)
            .style(self.theme.title_style());
        frame.render_widget(title, areas.title);

        // Username
        let username_focused = self.current_field == InputField::Username;
        let username = Paragraph::new(self.username_input.value()).block(
            Block::default()
                .title(" Username ")
                .borders(Borders::ALL)
                .border_style(if username_focused {
                    self.theme.focused_border_style()
                } else {
                    self.theme.inactive_style()
                }),
        );
        frame.render_widget(username, areas.username);

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
                    self.theme.inactive_style()
                }),
        );
        frame.render_widget(password, areas.password);

        // Help
        let help = Paragraph::new("Tab: 切替 | Enter: ログイン | Ctrl+C: 終了")
            .alignment(Alignment::Center)
            .style(self.theme.inactive_style());
        frame.render_widget(help, areas.help);
    }
}

/// Layout areas for the login form
struct FormAreas {
    title: Rect,
    username: Rect,
    password: Rect,
    help: Rect,
}

fn compute_form_areas(area: Rect) -> FormAreas {
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(10),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, form_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(50),
        Constraint::Fill(1),
    ])
    .areas(center);

    let [title_area, _gap, username_area, password_area, help_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(form_area);

    FormAreas {
        title: title_area,
        username: username_area,
        password: password_area,
        help: help_area,
    }
}

impl LoginComponent {
    /// Render the cursor at the current input field position.
    /// Call separately so callers can skip it when a popup is visible.
    pub fn render_cursor(&self, frame: &mut Frame, area: Rect) {
        let areas = compute_form_areas(area);
        let username_focused = self.current_field == InputField::Username;

        #[allow(clippy::cast_possible_truncation)]
        let (cursor_x, cursor_y) = if username_focused {
            (
                areas.username.x + self.username_input.visual_cursor() as u16 + 1,
                areas.username.y + 1,
            )
        } else {
            (
                areas.password.x + self.password_input.visual_cursor() as u16 + 1,
                areas.password.y + 1,
            )
        };
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<LoginAction> {
        match key.code {
            KeyCode::Tab => {
                self.current_field = match self.current_field {
                    InputField::Username => InputField::Password,
                    InputField::Password => InputField::Username,
                };
                None
            }
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
