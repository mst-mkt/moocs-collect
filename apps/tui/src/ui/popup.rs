use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme::Theme;

#[allow(dead_code)] // For future expansion
pub struct PopupConfig<'a> {
    pub title: Option<&'a str>,
    pub message: &'a str,
    pub width: u16,
    pub height: u16,
    pub wrap: bool,
}

#[allow(dead_code)] // For future expansion
impl<'a> PopupConfig<'a> {
    pub const fn new(message: &'a str) -> Self {
        Self {
            title: None,
            message,
            width: 40,
            height: 5,
            wrap: false,
        }
    }

    pub const fn with_title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub const fn with_size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub const fn with_wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}

fn centered_popup(area: Rect, width: u16, height: u16) -> Rect {
    let [_, center_v, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, popup_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(width),
        Constraint::Fill(1),
    ])
    .areas(center_v);

    popup_area
}

/// Render a loading popup
pub fn render_loading(frame: &mut Frame, message: &str, theme: &Theme) {
    let popup_area = centered_popup(frame.area(), 40, 5);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.focused_border_style());

    let paragraph = Paragraph::new(message)
        .block(block)
        .alignment(Alignment::Center)
        .style(theme.normal_style());

    frame.render_widget(paragraph, popup_area);
}

/// Render an error popup
pub fn render_error(frame: &mut Frame, message: &str, theme: &Theme) {
    let popup_area = centered_popup(frame.area(), 60, 8);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(theme.error_style())
        .title_style(theme.error_style());

    let content = format!("{message}\n\nPress ESC to dismiss");
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, popup_area);
}
