use collect::PageKey;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::Component;
use crate::ui::Theme;

/// Width of the progress bar in characters
const PROGRESS_BAR_WIDTH: usize = 20;

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
        // Item list - separator width accounts for borders(2) + highlight_symbol(2), with right margin
        let inner_width = area.width.saturating_sub(4) as usize;
        let separator_width = inner_width.saturating_sub(1); // right margin only
        let separator = "─".repeat(separator_width);

        let selected_idx = self.list_state.selected();

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let is_selected = selected_idx == Some(idx);

                let marker = if is_selected {
                    Span::styled(
                        "> ",
                        Style::default()
                            .fg(self.theme.primary)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw("  ")
                };

                let path_style = if is_selected {
                    Style::default()
                        .fg(self.theme.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.theme.normal_style()
                };
                let path_line =
                    Line::from(vec![marker, Span::styled(item.display_path(), path_style)]);

                let status_line = match &item.status {
                    DownloadStatus::Pending => {
                        let bar = format!("{}  待機中", "░".repeat(PROGRESS_BAR_WIDTH));
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(bar, self.theme.inactive_style()),
                        ])
                    }
                    DownloadStatus::Downloading(pct, phase) => {
                        let filled = (*pct as usize) * PROGRESS_BAR_WIDTH / 100;
                        let empty = PROGRESS_BAR_WIDTH - filled;
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                "█".repeat(filled),
                                Style::default().fg(self.theme.primary),
                            ),
                            Span::styled("░".repeat(empty), self.theme.dim_style()),
                            Span::styled(
                                format!("  {pct}% {phase}"),
                                Style::default().fg(self.theme.primary),
                            ),
                        ])
                    }
                    DownloadStatus::Completed => Line::from(vec![
                        Span::raw("  "),
                        Span::styled("█".repeat(PROGRESS_BAR_WIDTH), self.theme.success_style()),
                        Span::styled(" 100% 完了", self.theme.success_style()),
                    ]),
                    DownloadStatus::Failed(msg) => Line::from(vec![
                        Span::raw("  "),
                        Span::styled(format!("✗ エラー: {msg}"), self.theme.error_style()),
                    ]),
                };

                let sep_line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&*separator, self.theme.dim_style()),
                ]);

                ListItem::new(vec![path_line, status_line, sep_line])
            })
            .collect();

        // Build summary as title_bottom
        let summary_title = self.build_summary_title();

        let block = Block::default()
            .title(" ダウンロード ")
            .title_bottom(summary_title)
            .borders(Borders::ALL)
            .border_style(self.theme.focused_border_style());

        let list = List::new(list_items).block(block);

        frame.render_stateful_widget(list, area, &mut self.list_state.clone());
    }

    fn build_summary_title(&self) -> Line<'_> {
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
            Span::styled(" 全 ", self.theme.normal_style()),
            Span::styled(format!("{total}"), self.theme.title_style()),
            Span::styled(" 件", self.theme.normal_style()),
            Span::raw("  "),
            Span::styled("完了: ", self.theme.inactive_style()),
            Span::styled(format!("{completed}"), self.theme.success_style()),
        ];

        if downloading > 0 {
            spans.push(Span::styled("  実行中: ", self.theme.inactive_style()));
            spans.push(Span::styled(
                format!("{downloading}"),
                Style::default().fg(self.theme.primary),
            ));
        }

        if failed > 0 {
            spans.push(Span::styled("  失敗: ", self.theme.inactive_style()));
            spans.push(Span::styled(format!("{failed}"), self.theme.error_style()));
        }

        spans.push(Span::raw(" "));

        Line::from(spans)
    }
}
