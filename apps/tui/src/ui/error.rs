use ratatui::{
    layout::{Alignment, Constraint, Layout},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme::Theme;

pub fn render_error_popup(frame: &mut Frame, error_message: &str, theme: &Theme) {
    let area = frame.area();

    let [_, center_v, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, popup_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(60),
        Constraint::Fill(1),
    ])
    .areas(center_v);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Error")
        .borders(Borders::ALL)
        .border_style(theme.error_style())
        .title_style(theme.error_style());

    let content = format!("{error_message}\n\nPress ESC to dismiss");
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, popup_area);
}
