use collect::{
    Collect, Course, CourseKey, Credentials, Lecture, LectureKey, LecturePage, PageKey, Year,
};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame,
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Semaphore};

use crate::components::{
    login::LoginAction, selector::SelectorAction, Component, DownloadComponent, LoginComponent,
    SelectorComponent,
};
use crate::ui::{self, terminal, Theme, Tui};

const MAX_CONCURRENT_DOWNLOADS: usize = 5;
const MAX_HTTP_PERMITS: usize = 8;
/// Minimum footer width for responsive layout
const MIN_FOOTER_WIDTH: u16 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Login,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Selector,
    Download,
}

#[derive(Debug, Clone)]
pub enum AppAction {
    Login(Credentials),
    LoginSuccess,
    LoginFailed(String),
    FetchCourses,
    CoursesLoaded(Vec<Course>),
    CoursesFailed(String),
    FetchLectures(CourseKey),
    LecturesLoaded(CourseKey, Vec<Lecture>),
    LecturesFailed(String),
    FetchPages(LectureKey),
    PagesLoaded(LectureKey, Vec<LecturePage>),
    PagesFailed(String),
    SwitchTab(Tab),
    EnqueueDownloads,
    StartNextDownload,
    DownloadProgress(PageKey, u8, String),
    DownloadCompleted(PageKey),
    DownloadFailed(PageKey, String),
    ClearError,
    Quit,
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    screen: Screen,
    active_tab: Tab,
    error: Option<String>,
    running: bool,
    year: Option<u32>,
    download_path: PathBuf,
    login: LoginComponent,
    selector: SelectorComponent,
    download: DownloadComponent,
    theme: Theme,
    client: reqwest::Client,
    collect: Collect,
    semaphore: Arc<Semaphore>,
    action_tx: mpsc::Sender<AppAction>,
    action_rx: mpsc::Receiver<AppAction>,
    authenticating: bool,
    loading_courses: bool,
    pending_enqueue: bool,
}

impl App {
    pub fn new(download_path: Option<PathBuf>, year: Option<u32>) -> Self {
        let (action_tx, action_rx) = mpsc::channel(32);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0")
            .cookie_store(true)
            .build()
            .unwrap_or_default();
        let collect = Collect::from(Arc::new(client.clone()));

        Self {
            screen: Screen::default(),
            active_tab: Tab::default(),
            error: None,
            running: true,
            year,
            download_path: download_path.unwrap_or_else(|| PathBuf::from(".")),
            login: LoginComponent::new(),
            selector: SelectorComponent::new(),
            download: DownloadComponent::new(),
            theme: Theme::default(),
            client,
            collect,
            semaphore: Arc::new(Semaphore::new(MAX_HTTP_PERMITS)),
            action_tx,
            action_rx,
            authenticating: false,
            loading_courses: false,
            pending_enqueue: false,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = terminal::setup()?;
        let result = self.main_loop(&mut terminal);
        terminal::restore()?;
        result
    }

    fn main_loop(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            while let Ok(action) = self.action_rx.try_recv() {
                self.dispatch(action);
            }

            // Process all available events before next render
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers);
                    }
                }
            }

            // Sleep briefly to avoid busy-waiting when idle
            std::thread::sleep(Duration::from_millis(16));
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        let has_popup = self.authenticating || self.loading_courses || self.error.is_some();

        match self.screen {
            Screen::Login => {
                self.login.render(frame, area);
                if !has_popup {
                    self.login.render_cursor(frame, area);
                }
            }
            Screen::Main => {
                let [tab_area, content_area, footer_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Fill(1),
                    Constraint::Length(3),
                ])
                .areas(area);

                self.render_tab_bar(frame, tab_area);

                match self.active_tab {
                    Tab::Selector => self.selector.render(frame, content_area),
                    Tab::Download => self.download.render(frame, content_area),
                }

                self.render_footer(frame, footer_area);
            }
        }

        if self.authenticating {
            ui::render_loading(frame, "ログイン中...", &self.theme);
        }

        if self.loading_courses {
            ui::render_loading(frame, "科目を取得中...", &self.theme);
        }

        if let Some(ref error) = self.error {
            ui::render_error(frame, error, &self.theme);
        }
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let titles = vec![Line::from("講義一覧"), Line::from("ダウンロード")];
        let selected = match self.active_tab {
            Tab::Selector => 0,
            Tab::Download => 1,
        };

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title(" MOOCs Collect ")
                    .borders(Borders::ALL),
            )
            .select(selected)
            .style(self.theme.inactive_style())
            .highlight_style(self.theme.title_style())
            .divider("│");

        frame.render_widget(tabs, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let ks = self.theme.key_style();
        let ds = self.theme.inactive_style();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.dim_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let nav_spans = match self.active_tab {
            Tab::Selector => vec![
                Span::styled("↑↓", ks),
                Span::styled(": 移動", ds),
                Span::raw("  "),
                Span::styled("←→", ks),
                Span::styled(": カラム切替", ds),
                Span::raw("  "),
                Span::styled("Tab", ks),
                Span::styled(": タブ切替", ds),
            ],
            Tab::Download => vec![
                Span::styled("↑↓", ks),
                Span::styled(": 移動", ds),
                Span::raw("  "),
                Span::styled("Tab", ks),
                Span::styled(": タブ切替", ds),
            ],
        };

        let action_spans = match self.active_tab {
            Tab::Selector => vec![
                Span::styled("Space", ks),
                Span::styled(": 選択", ds),
                Span::raw("  "),
                Span::styled("Enter", ks),
                Span::styled(": 確定", ds),
                Span::raw("  "),
                Span::styled("q", ks),
                Span::styled(": 終了", ds),
            ],
            Tab::Download => vec![Span::styled("q", ks), Span::styled(": 終了", ds)],
        };

        // Responsive: use left-right layout if wide enough, fall back to simplified help
        if inner.width >= MIN_FOOTER_WIDTH {
            // Single row: navigation on the left, actions on the right
            let [left_area, right_area] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(inner);

            let mut left_spans = vec![Span::raw(" ")];
            left_spans.extend(nav_spans);

            let mut right_spans = action_spans;
            right_spans.push(Span::raw(" "));

            let left_line = ratatui::widgets::Paragraph::new(Line::from(left_spans));
            let right_line = ratatui::widgets::Paragraph::new(Line::from(right_spans))
                .alignment(ratatui::layout::Alignment::Right);

            frame.render_widget(left_line, left_area);
            frame.render_widget(right_line, right_area);
        } else {
            // Narrow screen: show simplified single-line help
            let simplified_spans = vec![
                Span::raw(" "),
                Span::styled("Tab", ks),
                Span::styled(": タブ", ds),
                Span::raw("  "),
                Span::styled("q", ks),
                Span::styled(": 終了", ds),
            ];
            let paragraph = ratatui::widgets::Paragraph::new(Line::from(simplified_spans));
            frame.render_widget(paragraph, inner);
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if self.authenticating {
            return;
        }

        if matches!(code, KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL))
            || matches!(code, KeyCode::Esc | KeyCode::Char('q'))
        {
            if self.error.is_some() {
                self.dispatch(AppAction::ClearError);
            } else {
                self.dispatch(AppAction::Quit);
            }
            return;
        }

        match self.screen {
            Screen::Login => {
                if let Some(LoginAction::Submit(creds)) = self
                    .login
                    .handle_event(Event::Key(event::KeyEvent::new(code, modifiers)))
                {
                    self.dispatch(AppAction::Login(creds));
                }
            }
            Screen::Main => {
                if code == KeyCode::Tab {
                    let new_tab = match self.active_tab {
                        Tab::Selector => Tab::Download,
                        Tab::Download => Tab::Selector,
                    };
                    self.dispatch(AppAction::SwitchTab(new_tab));
                    return;
                }

                match self.active_tab {
                    Tab::Selector => {
                        if let Some(action) = self
                            .selector
                            .handle_event(Event::Key(event::KeyEvent::new(code, modifiers)))
                        {
                            match action {
                                SelectorAction::FetchLectures(key) => {
                                    self.dispatch(AppAction::FetchLectures(key));
                                }
                                SelectorAction::FetchPages(key) => {
                                    self.dispatch(AppAction::FetchPages(key));
                                }
                                SelectorAction::Confirm => {
                                    self.dispatch(AppAction::EnqueueDownloads);
                                }
                            }
                        }
                    }
                    Tab::Download => {
                        let _ = self
                            .download
                            .handle_event(Event::Key(event::KeyEvent::new(code, modifiers)));
                    }
                }
            }
        }
    }

    fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::Login(creds) => {
                self.authenticating = true;
                self.error = None;
                self.start_authentication(creds);
            }
            AppAction::LoginSuccess => {
                self.authenticating = false;
                self.error = None;
                self.screen = Screen::Main;
                self.dispatch(AppAction::FetchCourses);
            }
            AppAction::LoginFailed(msg) => {
                self.authenticating = false;
                self.error = Some(msg);
            }
            AppAction::CoursesFailed(msg) => {
                self.loading_courses = false;
                self.error = Some(msg);
            }
            AppAction::LecturesFailed(msg) | AppAction::PagesFailed(msg) => {
                if !self.pending_enqueue && self.active_tab == Tab::Selector {
                    self.error = Some(msg);
                }
            }
            AppAction::FetchCourses => {
                self.loading_courses = true;
                self.start_fetch_courses();
            }
            AppAction::CoursesLoaded(courses) => {
                self.loading_courses = false;
                self.selector.set_courses(courses);
                if let Some(SelectorAction::FetchLectures(key)) =
                    self.selector.request_initial_data()
                {
                    self.dispatch(AppAction::FetchLectures(key));
                }
            }
            AppAction::FetchLectures(key) => {
                self.selector.set_loading_lectures(true);
                self.start_fetch_lectures(key);
            }
            AppAction::LecturesLoaded(course_key, lectures) => {
                self.selector.set_lectures(lectures, &course_key);
                if !self.selector.try_load_pages_from_cache() {
                    if let Some(key) = self.selector.get_focused_lecture_key() {
                        self.dispatch(AppAction::FetchPages(key));
                    }
                }
                if self.pending_enqueue {
                    self.try_enqueue_downloads();
                }
            }
            AppAction::FetchPages(key) => {
                self.selector.set_loading_pages(true);
                self.start_fetch_pages(key);
            }
            AppAction::PagesLoaded(lecture_key, pages) => {
                self.selector.set_pages(pages, &lecture_key);
                if self.pending_enqueue {
                    self.try_enqueue_downloads();
                }
            }
            AppAction::SwitchTab(tab) => {
                self.active_tab = tab;
            }
            AppAction::EnqueueDownloads => {
                self.try_enqueue_downloads();
            }
            AppAction::StartNextDownload => {
                self.fill_download_slots();
            }
            AppAction::DownloadProgress(page_key, progress, ref phase) => {
                self.download.update_progress(&page_key, progress, phase);
            }
            AppAction::DownloadCompleted(page_key) => {
                self.download.mark_completed(&page_key);
                self.dispatch(AppAction::StartNextDownload);
            }
            AppAction::DownloadFailed(page_key, msg) => {
                self.download.mark_failed(&page_key, msg);
                self.dispatch(AppAction::StartNextDownload);
            }
            AppAction::ClearError => {
                self.error = None;
            }
            AppAction::Quit => {
                self.running = false;
            }
        }
    }

    fn try_enqueue_downloads(&mut self) {
        let (courses_to_fetch, lectures_to_fetch) = self.selector.get_unfetched_for_selection();
        let has_missing = !courses_to_fetch.is_empty() || !lectures_to_fetch.is_empty();

        for key in courses_to_fetch {
            self.start_fetch_lectures(key);
        }
        for key in lectures_to_fetch {
            self.start_fetch_pages(key);
        }

        if has_missing {
            self.pending_enqueue = true;
            return;
        }

        self.pending_enqueue = false;
        self.error = None;
        let resolved = self.selector.resolve_selected_pages();
        if resolved.is_empty() {
            return;
        }
        self.download.enqueue(resolved);
        self.active_tab = Tab::Download;
        self.fill_download_slots();
    }

    fn start_authentication(&self, credentials: Credentials) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let result = collect.authenticate(&credentials).await;
            let action = match result {
                Ok(()) => AppAction::LoginSuccess,
                Err(e) => {
                    let msg = match e {
                        collect::error::CollectError::Authentication { reason } => {
                            format!("認証に失敗しました: {reason}")
                        }
                        _ => "ログインに失敗しました。ユーザー名とパスワードを確認してください"
                            .to_string(),
                    };
                    AppAction::LoginFailed(msg)
                }
            };
            let _ = tx.send(action).await;
        });
    }

    fn start_fetch_courses(&self) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();
        let year = self.year.and_then(|y| Year::new(y).ok());

        tokio::spawn(async move {
            let action = collect.get_courses(year).await.map_or_else(
                |_| AppAction::CoursesFailed("科目の取得に失敗しました".to_string()),
                AppAction::CoursesLoaded,
            );
            let _ = tx.send(action).await;
        });
    }

    fn start_fetch_lectures(&self, course_key: CourseKey) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let action = collect.get_lectures(&course_key).await.map_or_else(
                |_| AppAction::LecturesFailed("講義の取得に失敗しました".to_string()),
                |lectures| AppAction::LecturesLoaded(course_key, lectures),
            );
            let _ = tx.send(action).await;
        });
    }

    fn start_fetch_pages(&self, lecture_key: LectureKey) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let action = collect.get_pages(&lecture_key).await.map_or_else(
                |_| AppAction::PagesFailed("ページの取得に失敗しました".to_string()),
                |pages| AppAction::PagesLoaded(lecture_key, pages),
            );
            let _ = tx.send(action).await;
        });
    }

    fn fill_download_slots(&mut self) {
        while self.download.active_count() < MAX_CONCURRENT_DOWNLOADS {
            if let Some(page_key) = self.download.next_pending() {
                self.download.update_progress(&page_key, 0, "開始中...");
                self.start_download(page_key);
            } else {
                break;
            }
        }
    }

    fn start_download(&self, page_key: PageKey) {
        let collect = self.collect.clone();
        let client = self.client.clone();
        let tx = self.action_tx.clone();
        let download_path = self.download_path.clone();
        let semaphore = self.semaphore.clone();

        tokio::spawn(async move {
            let result = crate::download::download_page(
                &collect,
                &client,
                &page_key,
                &download_path,
                &tx,
                &semaphore,
            )
            .await;
            let action = match result {
                Ok(()) => AppAction::DownloadCompleted(page_key),
                Err(e) => AppAction::DownloadFailed(page_key, format!("{e:#}")),
            };
            let _ = tx.send(action).await;
        });
    }
}
