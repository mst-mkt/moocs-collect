use collect::{Collect, Course, CourseKey, Credentials, Lecture, LectureKey, LecturePage, PageKey, Year};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Line,
    widgets::{Block, Borders, Tabs},
    Frame,
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;

use crate::components::{
    login::LoginAction, selector::SelectorAction, Component, DownloadComponent, LoginComponent,
    SelectorComponent,
};
use crate::ui::{self, terminal, Theme, Tui};

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
    DownloadProgress(PageKey, u8),
    DownloadCompleted(PageKey),
    DownloadFailed(PageKey, String),
    ClearError,
    Quit,
}

pub struct App {
    screen: Screen,
    active_tab: Tab,
    error: Option<String>,
    running: bool,
    year: Option<u32>,
    login: LoginComponent,
    selector: SelectorComponent,
    download: DownloadComponent,
    theme: Theme,
    collect: Collect,
    action_tx: mpsc::Sender<AppAction>,
    action_rx: mpsc::Receiver<AppAction>,
    authenticating: bool,
    loading_courses: bool,
    pending_enqueue: bool,
}

impl App {
    pub fn new(_download_path: Option<PathBuf>, year: Option<u32>) -> Self {
        let (action_tx, action_rx) = mpsc::channel(32);
        let collect = Collect::default();

        Self {
            screen: Screen::default(),
            active_tab: Tab::default(),
            error: None,
            running: true,
            year,
            login: LoginComponent::new(),
            selector: SelectorComponent::new(),
            download: DownloadComponent::new(),
            theme: Theme::default(),
            collect,
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

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers);
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        match self.screen {
            Screen::Login => self.login.render(frame, area),
            Screen::Main => {
                let [tab_area, content_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Fill(1),
                ])
                .areas(area);

                self.render_tab_bar(frame, tab_area);

                match self.active_tab {
                    Tab::Selector => self.selector.render(frame, content_area),
                    Tab::Download => self.download.render(frame, content_area),
                }
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
        let titles = vec![Line::from(" 選択 "), Line::from(" ダウンロード ")];
        let selected = match self.active_tab {
            Tab::Selector => 0,
            Tab::Download => 1,
        };

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(selected)
            .style(self.theme.inactive_style())
            .highlight_style(self.theme.title_style())
            .divider("|");

        frame.render_widget(tabs, area);
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        if self.authenticating {
            return;
        }

        // Global: Quit
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
                // Tab switching
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
                if let Some(page_key) = self.download.next_pending() {
                    self.download.update_progress(&page_key, 0);
                    self.start_simulated_download(page_key);
                }
            }
            AppAction::DownloadProgress(page_key, progress) => {
                self.download.update_progress(&page_key, progress);
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
        let (courses_to_fetch, lectures_to_fetch) =
            self.selector.get_unfetched_for_selection();

        // Start fetching any missing data
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

        // All data is available — resolve and enqueue
        self.pending_enqueue = false;
        self.error = None;
        let resolved = self.selector.resolve_selected_pages();
        if resolved.is_empty() {
            return;
        }
        self.download.enqueue(resolved);
        self.active_tab = Tab::Download;
        if !self.download.has_active_download() {
            self.dispatch(AppAction::StartNextDownload);
        }
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
            let action = match collect.get_courses(year).await {
                Ok(courses) => AppAction::CoursesLoaded(courses),
                Err(_) => AppAction::CoursesFailed("科目の取得に失敗しました".to_string()),
            };
            let _ = tx.send(action).await;
        });
    }

    fn start_fetch_lectures(&self, course_key: CourseKey) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let action = match collect.get_lectures(&course_key).await {
                Ok(lectures) => AppAction::LecturesLoaded(course_key, lectures),
                Err(_) => AppAction::LecturesFailed("講義の取得に失敗しました".to_string()),
            };
            let _ = tx.send(action).await;
        });
    }

    fn start_fetch_pages(&self, lecture_key: LectureKey) {
        let collect = self.collect.clone();
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let action = match collect.get_pages(&lecture_key).await {
                Ok(pages) => AppAction::PagesLoaded(lecture_key, pages),
                Err(_) => AppAction::PagesFailed("ページの取得に失敗しました".to_string()),
            };
            let _ = tx.send(action).await;
        });
    }

    fn start_simulated_download(&self, page_key: PageKey) {
        let tx = self.action_tx.clone();

        tokio::spawn(async move {
            let steps = [20u8, 45, 70, 90];
            for progress in steps {
                tokio::time::sleep(Duration::from_millis(800)).await;
                let _ = tx
                    .send(AppAction::DownloadProgress(page_key.clone(), progress))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(800)).await;
            let _ = tx
                .send(AppAction::DownloadCompleted(page_key))
                .await;
        });
    }
}
