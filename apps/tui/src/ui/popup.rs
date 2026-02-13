use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::theme::Theme;

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

pub fn render_loading(frame: &mut Frame, message: &str, theme: &Theme) {
    let popup_area = centered_popup(frame.area(), 40, 5);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.focused_border_style());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let [_, text_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(theme.normal_style());

    frame.render_widget(paragraph, text_area);
}

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
