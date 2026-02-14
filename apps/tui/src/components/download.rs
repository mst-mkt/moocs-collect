use std::collections::HashMap;
use std::path::PathBuf;

use collect::PageKey;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use super::{move_list_selection, Component};
use crate::ui::Theme;

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
    Completed(Vec<PathBuf>),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum DownloadAction {
    Open(Vec<PathBuf>),
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub page_key: PageKey,
    pub display_path: String,
    pub status: DownloadStatus,
}

pub struct DownloadComponent {
    items: Vec<DownloadItem>,
    index: HashMap<PageKey, usize>,
    list_state: ListState,
}

impl Component for DownloadComponent {
    type Action = DownloadAction;

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.render_download_list(frame, area, theme);
    }
}

impl DownloadComponent {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            index: HashMap::new(),
            list_state: ListState::default(),
        }
    }

    pub fn enqueue(&mut self, pages: Vec<ResolvedPage>) {
        for page in pages {
            if self.index.contains_key(&page.page_key) {
                continue;
            }
            let display_path = format!(
                "{} > {} > {}",
                page.course_name, page.lecture_name, page.page_name
            );
            let idx = self.items.len();
            self.index.insert(page.page_key.clone(), idx);
            self.items.push(DownloadItem {
                page_key: page.page_key,
                display_path,
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
        if let Some(&idx) = self.index.get(page_key) {
            self.items[idx].status =
                DownloadStatus::Downloading(progress.min(100), phase.to_string());
        }
    }

    pub fn mark_completed(&mut self, page_key: &PageKey, files: Vec<PathBuf>) {
        if let Some(&idx) = self.index.get(page_key) {
            self.items[idx].status = DownloadStatus::Completed(files);
        }
    }

    pub fn mark_failed(&mut self, page_key: &PageKey, error: String) {
        if let Some(&idx) = self.index.get(page_key) {
            self.items[idx].status = DownloadStatus::Failed(error);
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<DownloadAction> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_list_selection(&mut self.list_state, self.items.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_list_selection(&mut self.list_state, self.items.len(), 1);
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(item) = self.items.get(idx) {
                        if let DownloadStatus::Completed(ref files) = item.status {
                            if !files.is_empty() {
                                return Some(DownloadAction::Open(files.clone()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn render_download_list(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let inner_width = area.width.saturating_sub(4) as usize;
        let separator_width = inner_width.saturating_sub(1);
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
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw("  ")
                };

                let path_style = if is_selected {
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme.normal_style()
                };
                let path_line =
                    Line::from(vec![marker, Span::styled(&*item.display_path, path_style)]);

                let status_line = match &item.status {
                    DownloadStatus::Pending => {
                        let bar = format!("{}  待機中", "░".repeat(PROGRESS_BAR_WIDTH));
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(bar, theme.inactive_style()),
                        ])
                    }
                    DownloadStatus::Downloading(pct, phase) => {
                        let filled = (*pct as usize) * PROGRESS_BAR_WIDTH / 100;
                        let empty = PROGRESS_BAR_WIDTH - filled;
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled("█".repeat(filled), Style::default().fg(theme.primary)),
                            Span::styled("░".repeat(empty), theme.dim_style()),
                            Span::styled(
                                format!("  {pct}% {phase}"),
                                Style::default().fg(theme.primary),
                            ),
                        ])
                    }
                    DownloadStatus::Completed(_) => Line::from(vec![
                        Span::raw("  "),
                        Span::styled("█".repeat(PROGRESS_BAR_WIDTH), theme.success_style()),
                        Span::styled(" 100% 完了", theme.success_style()),
                    ]),
                    DownloadStatus::Failed(msg) => Line::from(vec![
                        Span::raw("  "),
                        Span::styled(format!("✗ エラー: {msg}"), theme.error_style()),
                    ]),
                };

                let sep_line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&*separator, theme.dim_style()),
                ]);

                ListItem::new(vec![path_line, status_line, sep_line])
            })
            .collect();

        let summary_title = build_summary_title(&self.items, theme);

        let block = Block::default()
            .title(" ダウンロード ")
            .title_bottom(summary_title)
            .borders(Borders::ALL)
            .border_style(theme.focused_border_style());

        let list = List::new(list_items).block(block);

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn build_summary_title<'a>(items: &[DownloadItem], theme: &'a Theme) -> Line<'a> {
    let total = items.len();
    let completed = items
        .iter()
        .filter(|i| matches!(i.status, DownloadStatus::Completed(_)))
        .count();
    let downloading = items
        .iter()
        .filter(|i| matches!(i.status, DownloadStatus::Downloading(..)))
        .count();
    let failed = items
        .iter()
        .filter(|i| matches!(i.status, DownloadStatus::Failed(_)))
        .count();

    let mut spans = vec![
        Span::styled(" 全 ", theme.normal_style()),
        Span::styled(format!("{total}"), theme.title_style()),
        Span::styled(" 件", theme.normal_style()),
        Span::raw("  "),
        Span::styled("完了: ", theme.inactive_style()),
        Span::styled(format!("{completed}"), theme.success_style()),
    ];

    if downloading > 0 {
        spans.push(Span::styled("  実行中: ", theme.inactive_style()));
        spans.push(Span::styled(
            format!("{downloading}"),
            Style::default().fg(theme.primary),
        ));
    }

    if failed > 0 {
        spans.push(Span::styled("  失敗: ", theme.inactive_style()));
        spans.push(Span::styled(format!("{failed}"), theme.error_style()));
    }

    spans.push(Span::raw(" "));

    Line::from(spans)
}
