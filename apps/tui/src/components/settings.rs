use collect::Year;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;
use tui_input::{backend::crossterm::EventHandler, Input};

use super::Component;
use crate::ui::Theme;

const MAX_CONCURRENCY: usize = 8;

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Year(Option<u32>),
    Path(PathBuf),
    Concurrency(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    Year,
    Path,
    Concurrency,
}

pub struct SettingsComponent {
    available_years: Vec<Year>,
    selected_year_index: Option<usize>,

    path_input: Input,
    path_editing: bool,

    concurrency: usize,

    current_section: SettingsSection,
}

impl Component for SettingsComponent {
    type Action = SettingsAction;

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let outer_block = Block::default()
            .title(" 設定 ")
            .title_style(theme.title_style())
            .borders(Borders::ALL)
            .border_style(theme.dim_style());
        let inner = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        // Padding: top 1, left 2, right 2
        let [_, padded] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
        let [_, content_area, _] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(padded);

        // label(1) + value(1) + separator(1) per section
        let [
            year_label, year_value, sep1,
            path_label, path_value, sep2,
            conc_label, conc_value, _rest,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(content_area);

        let year_display = self
            .selected_year_index
            .and_then(|i| self.available_years.get(i))
            .map_or_else(
                || "読み込み中...".to_string(),
                |y| format!("{y}年度"),
            );
        let year_focused = self.current_section == SettingsSection::Year;
        render_selector_section(frame, year_label, year_value, "年度", &year_display, year_focused, theme);

        render_separator(frame, sep1, theme);
        self.render_path_section(frame, path_label, path_value, theme);
        render_separator(frame, sep2, theme);

        let conc_focused = self.current_section == SettingsSection::Concurrency;
        render_selector_section(frame, conc_label, conc_value, "並行ダウンロード数", &self.concurrency.to_string(), conc_focused, theme);
    }
}

impl SettingsComponent {
    pub fn new(download_path: &std::path::Path, _year: Option<u32>, concurrency: usize) -> Self {
        let path_str = download_path.to_string_lossy().to_string();
        Self {
            available_years: Vec::new(),
            selected_year_index: None,
            path_input: Input::new(path_str),
            path_editing: false,
            concurrency,
            current_section: SettingsSection::default(),
        }
    }

    pub fn set_available_years(&mut self, years: Vec<Year>) {
        self.available_years = years;
        if self.selected_year_index.is_none() && !self.available_years.is_empty() {
            self.selected_year_index = Some(0);
        }
    }

    pub const fn is_editing(&self) -> bool {
        self.path_editing
    }

    fn selected_year_value(&self) -> Option<u32> {
        self.selected_year_index
            .and_then(|i| self.available_years.get(i))
            .and_then(|y| y.to_string().parse::<u32>().ok())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<SettingsAction> {
        if self.path_editing {
            return self.handle_path_editing(key);
        }
        self.handle_navigation(key)
    }

    fn handle_path_editing(&mut self, key: KeyEvent) -> Option<SettingsAction> {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.path_editing = false;
                let path = PathBuf::from(self.path_input.value());
                Some(SettingsAction::Path(path))
            }
            _ => {
                self.path_input.handle_event(&Event::Key(key));
                None
            }
        }
    }

    fn handle_navigation(&mut self, key: KeyEvent) -> Option<SettingsAction> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.current_section = match self.current_section {
                    SettingsSection::Year | SettingsSection::Path => SettingsSection::Year,
                    SettingsSection::Concurrency => SettingsSection::Path,
                };
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.current_section = match self.current_section {
                    SettingsSection::Year => SettingsSection::Path,
                    SettingsSection::Path | SettingsSection::Concurrency => {
                        SettingsSection::Concurrency
                    }
                };
                None
            }
            KeyCode::Left | KeyCode::Char('h') => match self.current_section {
                SettingsSection::Year => self.cycle_year(-1),
                SettingsSection::Concurrency => self.cycle_concurrency(-1),
                SettingsSection::Path => None,
            },
            KeyCode::Right | KeyCode::Char('l') => match self.current_section {
                SettingsSection::Year => self.cycle_year(1),
                SettingsSection::Concurrency => self.cycle_concurrency(1),
                SettingsSection::Path => None,
            },
            KeyCode::Enter | KeyCode::Char(' ')
                if self.current_section == SettingsSection::Path =>
            {
                self.path_editing = true;
                None
            }
            _ => None,
        }
    }

    fn cycle_year(&mut self, delta: i32) -> Option<SettingsAction> {
        if self.available_years.is_empty() {
            return None;
        }
        let len = self.available_years.len();
        let current = self.selected_year_index.unwrap_or(0);
        #[allow(clippy::cast_sign_loss)]
        let next = if delta > 0 {
            (current + delta as usize).min(len - 1)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        if next != current {
            self.selected_year_index = Some(next);
            return Some(SettingsAction::Year(self.selected_year_value()));
        }
        None
    }

    fn cycle_concurrency(&mut self, delta: i32) -> Option<SettingsAction> {
        let next = if delta > 0 {
            (self.concurrency + 1).min(MAX_CONCURRENCY)
        } else {
            self.concurrency.saturating_sub(1).max(1)
        };
        if next != self.concurrency {
            self.concurrency = next;
            return Some(SettingsAction::Concurrency(next));
        }
        None
    }

    fn render_path_section(
        &self,
        frame: &mut Frame,
        label_area: Rect,
        value_area: Rect,
        theme: &Theme,
    ) {
        let focused = self.current_section == SettingsSection::Path;
        let active = focused || self.path_editing;
        let label_style = if active {
            theme.title_style()
        } else {
            theme.inactive_style()
        };
        let bracket_style = if active {
            theme.focused_border_style()
        } else {
            theme.dim_style()
        };
        let value_style = if active {
            theme.normal_style()
        } else {
            theme.inactive_style()
        };

        frame.render_widget(
            Paragraph::new(Span::styled("ダウンロードパス", label_style)),
            label_area,
        );

        let value_line = Line::from(vec![
            Span::styled("[ ", bracket_style),
            Span::styled(self.path_input.value(), value_style),
            Span::styled(" ]", bracket_style),
        ]);
        frame.render_widget(Paragraph::new(value_line), value_area);

        if self.path_editing {
            #[allow(clippy::cast_possible_truncation)]
            let cursor_x = value_area.x + 2 + self.path_input.visual_cursor() as u16;
            frame.set_cursor_position((cursor_x, value_area.y));
        }
    }
}

fn render_selector_section(
    frame: &mut Frame,
    label_area: Rect,
    value_area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    theme: &Theme,
) {
    let label_style = if focused {
        theme.title_style()
    } else {
        theme.inactive_style()
    };
    let bracket_style = if focused {
        theme.focused_border_style()
    } else {
        theme.dim_style()
    };
    let arrow_style = if focused {
        theme.key_style()
    } else {
        theme.dim_style()
    };
    let text_style = if focused {
        theme.normal_style()
    } else {
        theme.inactive_style()
    };

    frame.render_widget(
        Paragraph::new(Span::styled(label, label_style)),
        label_area,
    );

    let value_line = Line::from(vec![
        Span::styled("[ ", bracket_style),
        Span::styled("◀ ", arrow_style),
        Span::styled(value, text_style),
        Span::styled(" ▶", arrow_style),
        Span::styled(" ]", bracket_style),
    ]);
    frame.render_widget(Paragraph::new(value_line), value_area);
}

fn render_separator(frame: &mut Frame, area: Rect, theme: &Theme) {
    let line = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(line, theme.dim_style())),
        area,
    );
}
