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
    Submit(Credentials, bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputField {
    #[default]
    Username,
    Password,
    Remember,
}

pub struct LoginComponent {
    username_input: Input,
    password_input: Input,
    current_field: InputField,
    remember: bool,
}

impl Component for LoginComponent {
    type Action = LoginAction;

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let areas = compute_form_areas(area);

        let title = Paragraph::new("MOOCs Collect")
            .alignment(Alignment::Center)
            .style(theme.title_style());
        frame.render_widget(title, areas.title);

        let username_focused = self.current_field == InputField::Username;
        let username = Paragraph::new(self.username_input.value()).block(
            Block::default()
                .title(" Username ")
                .borders(Borders::ALL)
                .border_style(if username_focused {
                    theme.focused_border_style()
                } else {
                    theme.inactive_style()
                }),
        );
        frame.render_widget(username, areas.username);

        let password_focused = self.current_field == InputField::Password;
        let password_display = "*".repeat(self.password_input.value().len());
        let password = Paragraph::new(password_display).block(
            Block::default()
                .title(" Password ")
                .borders(Borders::ALL)
                .border_style(if password_focused {
                    theme.focused_border_style()
                } else {
                    theme.inactive_style()
                }),
        );
        frame.render_widget(password, areas.password);

        let remember_focused = self.current_field == InputField::Remember;
        let checkbox = if self.remember { "[x]" } else { "[ ]" };
        let remember_text = format!("{checkbox} 認証情報を保持する");
        let remember_style = if remember_focused {
            theme.title_style()
        } else {
            theme.inactive_style()
        };
        let remember = Paragraph::new(remember_text).style(remember_style);
        frame.render_widget(remember, areas.remember);

        let help = Paragraph::new("Tab: 切替 | Enter: 決定 | Ctrl+C: 終了")
            .alignment(Alignment::Center)
            .style(theme.inactive_style());
        frame.render_widget(help, areas.help);
    }
}

struct FormAreas {
    title: Rect,
    username: Rect,
    password: Rect,
    remember: Rect,
    help: Rect,
}

fn compute_form_areas(area: Rect) -> FormAreas {
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(12),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, form_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(50),
        Constraint::Fill(1),
    ])
    .areas(center);

    let [title_area, _gap, username_area, password_area, remember_area, _gap2, help_area] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .areas(form_area);

    FormAreas {
        title: title_area,
        username: username_area,
        password: password_area,
        remember: remember_area,
        help: help_area,
    }
}

impl LoginComponent {
    pub fn new() -> Self {
        Self {
            username_input: Input::default(),
            password_input: Input::default(),
            current_field: InputField::default(),
            remember: false,
        }
    }

    pub fn set_username(&mut self, username: &str) {
        self.username_input = Input::new(username.to_string());
    }

    pub fn set_password(&mut self, password: &str) {
        self.password_input = Input::new(password.to_string());
    }

    pub const fn set_remember(&mut self, remember: bool) {
        self.remember = remember;
    }

    pub fn render_cursor(&self, frame: &mut Frame, area: Rect) {
        let areas = compute_form_areas(area);

        #[allow(clippy::cast_possible_truncation)]
        match self.current_field {
            InputField::Username => {
                let cursor_x = areas.username.x + self.username_input.visual_cursor() as u16 + 1;
                let cursor_y = areas.username.y + 1;
                frame.set_cursor_position((cursor_x, cursor_y));
            }
            InputField::Password => {
                let cursor_x = areas.password.x + self.password_input.visual_cursor() as u16 + 1;
                let cursor_y = areas.password.y + 1;
                frame.set_cursor_position((cursor_x, cursor_y));
            }
            InputField::Remember => {}
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<LoginAction> {
        match key.code {
            KeyCode::Tab => {
                self.current_field = match self.current_field {
                    InputField::Username => InputField::Password,
                    InputField::Password => InputField::Remember,
                    InputField::Remember => InputField::Username,
                };
                None
            }
            KeyCode::BackTab => {
                self.current_field = match self.current_field {
                    InputField::Username => InputField::Remember,
                    InputField::Password => InputField::Username,
                    InputField::Remember => InputField::Password,
                };
                None
            }
            KeyCode::Enter => match self.current_field {
                InputField::Username => {
                    self.current_field = InputField::Password;
                    None
                }
                InputField::Password | InputField::Remember => self.try_submit(),
            },
            KeyCode::Char(' ') if self.current_field == InputField::Remember => {
                self.remember = !self.remember;
                None
            }
            _ => {
                match self.current_field {
                    InputField::Username => {
                        self.username_input.handle_event(&Event::Key(key));
                    }
                    InputField::Password => {
                        self.password_input.handle_event(&Event::Key(key));
                    }
                    InputField::Remember => {}
                }
                None
            }
        }
    }

    fn try_submit(&self) -> Option<LoginAction> {
        let username = self.username_input.value().trim().to_string();
        let password = self.password_input.value().to_string();
        if !username.is_empty() && !password.is_empty() {
            Some(LoginAction::Submit(
                Credentials { username, password },
                self.remember,
            ))
        } else {
            None
        }
    }
}
