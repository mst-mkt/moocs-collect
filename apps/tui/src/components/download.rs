use collect::PageKey;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::Component;
use crate::ui::Theme;

#[derive(Debug, Clone)]
pub struct ResolvedPage {
    pub page_key: PageKey,
    pub course_name: String,
    pub lecture_name: String,
    pub page_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading(u8, String),
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub page_key: PageKey,
    pub course_name: String,
    pub lecture_name: String,
    pub page_name: String,
    pub status: DownloadStatus,
}

impl DownloadItem {
    fn display_path(&self) -> String {
        format!(
            "{} > {} > {}",
            self.course_name, self.lecture_name, self.page_name
        )
    }
}

#[derive(Debug, Clone)]
pub enum DownloadAction {}

pub struct DownloadComponent {
    items: Vec<DownloadItem>,
    list_state: ListState,
    theme: Theme,
}

impl Component for DownloadComponent {
    type Action = DownloadAction;

    fn new() -> Self {
        Self {
            items: Vec::new(),
            list_state: ListState::default(),
            theme: Theme::default(),
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            self.handle_key_event(key);
        }
        None
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_download_list(frame, area);
    }
}

impl DownloadComponent {
    pub fn enqueue(&mut self, pages: Vec<ResolvedPage>) {
        for page in pages {
            if self.items.iter().any(|item| item.page_key == page.page_key) {
                continue;
            }
            self.items.push(DownloadItem {
                page_key: page.page_key,
                course_name: page.course_name,
                lecture_name: page.lecture_name,
                page_name: page.page_name,
                status: DownloadStatus::Pending,
            });
        }
        if !self.items.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
    }

    pub fn next_pending(&self) -> Option<PageKey> {
        self.items
            .iter()
            .find(|item| item.status == DownloadStatus::Pending)
            .map(|item| item.page_key.clone())
    }

    pub fn active_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item.status, DownloadStatus::Downloading(..)))
            .count()
    }

    pub fn update_progress(&mut self, page_key: &PageKey, progress: u8, phase: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| &i.page_key == page_key) {
            item.status = DownloadStatus::Downloading(progress.min(100), phase.to_string());
        }
    }

    pub fn mark_completed(&mut self, page_key: &PageKey) {
        if let Some(item) = self.items.iter_mut().find(|i| &i.page_key == page_key) {
            item.status = DownloadStatus::Completed;
        }
    }

    pub fn mark_failed(&mut self, page_key: &PageKey, error: String) {
        if let Some(item) = self.items.iter_mut().find(|i| &i.page_key == page_key) {
            item.status = DownloadStatus::Failed(error);
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let next = if delta > 0 {
            current.saturating_add(delta as usize).min(len - 1)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        self.list_state.select(Some(next));
    }

    fn render_download_list(&self, frame: &mut Frame, area: Rect) {
        let [summary_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

        // Summary bar
        self.render_summary(frame, summary_area);

        // Item list
        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let path_line =
                    Line::from(Span::styled(item.display_path(), self.theme.normal_style()));

                let status_line = match &item.status {
                    DownloadStatus::Pending => {
                        Line::from(Span::styled("  待機中", self.theme.inactive_style()))
                    }
                    DownloadStatus::Downloading(pct, phase) => {
                        let filled = (*pct as usize) / 5;
                        let empty = 20 - filled;
                        let bar = format!(
                            "  [{}{}] {}% {phase}",
                            "#".repeat(filled),
                            ".".repeat(empty),
                            pct
                        );
                        Line::from(Span::styled(bar, Style::default().fg(self.theme.primary)))
                    }
                    DownloadStatus::Completed => Line::from(Span::styled(
                        "  [####################] 100% 完了",
                        self.theme.success_style(),
                    )),
                    DownloadStatus::Failed(msg) => Line::from(Span::styled(
                        format!("  エラー: {msg}"),
                        self.theme.error_style(),
                    )),
                };

                ListItem::new(vec![path_line, status_line, Line::default()])
            })
            .collect();

        let block = Block::default()
            .title(" ダウンロード ")
            .borders(Borders::ALL)
            .border_style(self.theme.focused_border_style());

        let list = List::new(list_items)
            .block(block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .fg(self.theme.primary),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, list_area, &mut self.list_state.clone());
    }

    fn render_summary(&self, frame: &mut Frame, area: Rect) {
        let total = self.items.len();
        let completed = self
            .items
            .iter()
            .filter(|i| i.status == DownloadStatus::Completed)
            .count();
        let downloading = self
            .items
            .iter()
            .filter(|i| matches!(i.status, DownloadStatus::Downloading(..)))
            .count();
        let failed = self
            .items
            .iter()
            .filter(|i| matches!(i.status, DownloadStatus::Failed(_)))
            .count();

        let mut spans = vec![
            Span::styled(format!(" 全{total}件"), self.theme.normal_style()),
            Span::styled("  ", Style::default()),
            Span::styled(format!("完了: {completed}"), self.theme.success_style()),
        ];

        if downloading > 0 {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(
                format!("実行中: {downloading}"),
                Style::default().fg(self.theme.primary),
            ));
        }

        if failed > 0 {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(
                format!("失敗: {failed}"),
                self.theme.error_style(),
            ));
        }

        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }
}
