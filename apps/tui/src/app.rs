use collect::{Collect, Course, CourseKey, Credentials, Lecture, LectureKey, LecturePage, PageKey};
use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs},
    Frame,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, Semaphore};

use crate::components::{
    login::LoginAction, selector::SelectorAction, Component, DownloadComponent, LoginComponent,
    SelectorComponent,
};
use crate::error::TuiError;
use crate::service::AppService;
use crate::state::{AppState, LoginPhase, LoginState, MainState, SelectorPhase, Tab};
use crate::ui::{self, terminal, Theme, Tui};

const MAX_HTTP_PERMITS: usize = 8;
const MIN_FOOTER_WIDTH: u16 = 60;

#[derive(Debug, Clone)]
pub enum AppAction {
    Login(Credentials),
    LoginSuccess,

    FetchCourses,
    CoursesLoaded(Vec<Course>),
    FetchLectures(CourseKey),
    LecturesLoaded(CourseKey, Vec<Lecture>),
    FetchPages(LectureKey),
    PagesLoaded(LectureKey, Vec<LecturePage>),

    SwitchTab(Tab),
    EnqueueDownloads,
    StartNextDownload,

    DownloadProgress(PageKey, u8, String),
    DownloadCompleted(PageKey),
    DownloadFailed(PageKey, String),

    Error(TuiError),

    ClearError,
    Quit,
}

pub struct App {
    state: AppState,
    error: Option<String>,
    running: bool,
    concurrency: usize,
    login: LoginComponent,
    selector: SelectorComponent,
    download: DownloadComponent,
    theme: Theme,
    service: AppService,
    action_rx: mpsc::Receiver<AppAction>,
}

impl App {
    pub fn new(download_path: Option<PathBuf>, year: Option<u32>, concurrency: usize) -> Self {
        let (action_tx, action_rx) = mpsc::channel(32);

        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0")
            .cookie_store(true)
            .build()
            .unwrap_or_default();
        let collect = Collect::from(Arc::new(client.clone()));
        let download_path = download_path.unwrap_or_else(|| PathBuf::from("."));

        let service = AppService::new(
            collect,
            client,
            Arc::new(Semaphore::new(MAX_HTTP_PERMITS)),
            action_tx,
            download_path,
            year,
        );

        Self {
            state: AppState::default(),
            error: None,
            running: true,
            concurrency,
            login: LoginComponent::new(),
            selector: SelectorComponent::new(),
            download: DownloadComponent::new(),
            theme: Theme::default(),
            service,
            action_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut terminal = terminal::setup()?;
        let result = self.main_loop(&mut terminal).await;
        terminal::restore()?;
        result
    }

    async fn main_loop(&mut self, terminal: &mut Tui) -> Result<()> {
        let mut event_stream = EventStream::new();

        terminal.draw(|frame| self.render(frame))?;

        loop {
            tokio::select! {
                Some(action) = self.action_rx.recv() => {
                    self.dispatch(action);
                    while let Ok(action) = self.action_rx.try_recv() {
                        self.dispatch(action);
                    }
                }
                Some(Ok(event)) = event_stream.next() => {
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press {
                            self.handle_key(key);
                        }
                    }
                }
            }

            if !self.running {
                break;
            }

            terminal.draw(|frame| self.render(frame))?;
        }

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        match &self.state {
            AppState::Login(_) => {
                self.login.render(frame, area, &self.theme);
                if self.state.can_handle_input() && self.error.is_none() {
                    self.login.render_cursor(frame, area);
                }
            }
            AppState::Main(main_state) => {
                let [tab_area, content_area, footer_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Fill(1),
                    Constraint::Length(3),
                ])
                .areas(area);

                self.render_tab_bar(frame, tab_area);

                match main_state.active_tab {
                    Tab::Selector => self.selector.render(frame, content_area, &self.theme),
                    Tab::Download => self.download.render(frame, content_area, &self.theme),
                }

                self.render_footer(frame, footer_area);
            }
        }

        if let Some(message) = self.state.loading_message() {
            ui::render_loading(frame, message, &self.theme);
        }

        if let Some(ref error) = self.error {
            ui::render_error(frame, error, &self.theme);
        }
    }

    fn render_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let titles = vec![Line::from("講義一覧"), Line::from("ダウンロード")];

        let selected = if let AppState::Main(main_state) = &self.state {
            match main_state.active_tab {
                Tab::Selector => 0,
                Tab::Download => 1,
            }
        } else {
            0
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

        let active_tab = if let AppState::Main(main_state) = &self.state {
            main_state.active_tab
        } else {
            Tab::Selector
        };

        let nav_spans = match active_tab {
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

        let action_spans = match active_tab {
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

        if inner.width >= MIN_FOOTER_WIDTH {
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

    fn handle_key(&mut self, key: KeyEvent) {
        if !self.state.can_handle_input() {
            return;
        }

        let code = key.code;
        let modifiers = key.modifiers;

        if matches!(code, KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL)) {
            self.dispatch(AppAction::Quit);
            return;
        }

        if self.error.is_some() {
            if matches!(code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
                self.dispatch(AppAction::ClearError);
            }
            return;
        }

        match &self.state {
            AppState::Login(_) => {
                if let Some(LoginAction::Submit(creds)) = self.login.handle_event(Event::Key(key)) {
                    self.dispatch(AppAction::Login(creds));
                }
            }
            AppState::Main(main_state) => {
                if matches!(code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.dispatch(AppAction::Quit);
                    return;
                }

                if code == KeyCode::Tab {
                    let new_tab = match main_state.active_tab {
                        Tab::Selector => Tab::Download,
                        Tab::Download => Tab::Selector,
                    };
                    self.dispatch(AppAction::SwitchTab(new_tab));
                    return;
                }

                match main_state.active_tab {
                    Tab::Selector => {
                        if let Some(action) = self.selector.handle_event(Event::Key(key)) {
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
                        let _ = self.download.handle_event(Event::Key(key));
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::Login(creds) => {
                self.state = AppState::Login(LoginState {
                    phase: LoginPhase::Authenticating,
                });
                self.error = None;
                self.service.authenticate(creds);
            }
            AppAction::LoginSuccess => {
                self.state = AppState::Main(MainState {
                    active_tab: Tab::Selector,
                    selector_phase: SelectorPhase::Idle,
                });
                self.error = None;
                self.dispatch(AppAction::FetchCourses);
            }

            AppAction::FetchCourses => {
                if let AppState::Main(ref mut main) = self.state {
                    main.selector_phase = SelectorPhase::LoadingCourses;
                }
                self.service.fetch_courses();
            }
            AppAction::CoursesLoaded(courses) => {
                if let AppState::Main(ref mut main) = self.state {
                    main.selector_phase = SelectorPhase::Ready;
                }
                self.selector.set_courses(courses);
                if let Some(SelectorAction::FetchLectures(key)) =
                    self.selector.request_initial_data()
                {
                    self.dispatch(AppAction::FetchLectures(key));
                }
            }
            AppAction::FetchLectures(key) => {
                self.selector.set_loading_lectures(true);
                self.service.fetch_lectures(key);
            }
            AppAction::LecturesLoaded(course_key, lectures) => {
                self.selector.set_lectures(lectures, &course_key);
                if !self.selector.try_load_pages_from_cache() {
                    if let Some(key) = self.selector.get_focused_lecture_key() {
                        self.dispatch(AppAction::FetchPages(key));
                    }
                }
                if let AppState::Main(MainState {
                    selector_phase: SelectorPhase::EnqueuePending,
                    ..
                }) = &self.state
                {
                    self.try_enqueue_downloads();
                }
            }
            AppAction::FetchPages(key) => {
                self.selector.set_loading_pages(true);
                self.service.fetch_pages(key);
            }
            AppAction::PagesLoaded(lecture_key, pages) => {
                self.selector.set_pages(pages, &lecture_key);
                if let AppState::Main(MainState {
                    selector_phase: SelectorPhase::EnqueuePending,
                    ..
                }) = &self.state
                {
                    self.try_enqueue_downloads();
                }
            }

            AppAction::SwitchTab(tab) => {
                if let AppState::Main(ref mut main) = self.state {
                    main.active_tab = tab;
                }
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

            AppAction::Error(err) => {
                match &mut self.state {
                    AppState::Login(ref mut login) => {
                        login.phase = LoginPhase::Input;
                    }
                    AppState::Main(ref mut main) => {
                        main.selector_phase = SelectorPhase::Idle;
                    }
                }
                self.error = Some(err.user_message());
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
            self.service.fetch_lectures(key);
        }
        for key in lectures_to_fetch {
            self.service.fetch_pages(key);
        }

        if has_missing {
            if let AppState::Main(ref mut main) = self.state {
                main.selector_phase = SelectorPhase::EnqueuePending;
            }
            return;
        }

        if let AppState::Main(ref mut main) = self.state {
            main.selector_phase = SelectorPhase::Idle;
        }
        self.error = None;
        let resolved = self.selector.resolve_selected_pages();
        if resolved.is_empty() {
            return;
        }
        self.download.enqueue(resolved);
        if let AppState::Main(ref mut main) = self.state {
            main.active_tab = Tab::Download;
        }
        self.fill_download_slots();
    }

    fn fill_download_slots(&mut self) {
        while self.download.active_count() < self.concurrency {
            if let Some(page_key) = self.download.next_pending() {
                self.download.update_progress(&page_key, 0, "開始中...");
                self.service.download(page_key);
            } else {
                break;
            }
        }
    }
}
