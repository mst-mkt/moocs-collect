use collect::{Collect, Course, CourseKey, Credentials, Lecture, LectureKey, LecturePage, Year};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;

use crate::components::{
    login::LoginAction, selector::SelectorAction, Component, LoginComponent, SelectorComponent,
};
use crate::ui::{self, terminal, Theme, Tui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Login,
    Selector,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
    Navigate(Screen),
    ShowError(String),
    ClearError,
    Quit,
}

#[derive(Debug)]
enum AsyncResult {
    AuthSuccess,
    AuthFailed(String),
    Courses(Vec<Course>),
    CoursesFailed(String),
    Lectures(CourseKey, Vec<Lecture>),
    LecturesFailed(String),
    Pages(LectureKey, Vec<LecturePage>),
    PagesFailed(String),
}

pub struct App {
    screen: Screen,
    error: Option<String>,
    running: bool,
    year: Option<u32>,
    login: LoginComponent,
    selector: SelectorComponent,
    theme: Theme,
    collect: Collect,
    async_tx: mpsc::Sender<AsyncResult>,
    async_rx: mpsc::Receiver<AsyncResult>,
    authenticating: bool,
    loading_courses: bool,
}

impl App {
    pub fn new(_download_path: Option<PathBuf>, year: Option<u32>) -> Self {
        let (async_tx, async_rx) = mpsc::channel(4);
        let collect = Collect::default();

        Self {
            screen: Screen::default(),
            error: None,
            running: true,
            year,
            login: LoginComponent::new(),
            selector: SelectorComponent::new(),
            theme: Theme::default(),
            collect,
            async_tx,
            async_rx,
            authenticating: false,
            loading_courses: false,
        }
    }

    #[allow(clippy::unused_async)]
    pub async fn run(&mut self) -> Result<()> {
        let mut terminal = terminal::setup()?;
        let result = self.main_loop(&mut terminal);
        terminal::restore()?;
        result
    }

    fn main_loop(&mut self, terminal: &mut Tui) -> Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            while let Ok(result) = self.async_rx.try_recv() {
                match result {
                    AsyncResult::AuthSuccess => {
                        self.authenticating = false;
                        self.login.set_loading(false);
                        self.dispatch(AppAction::LoginSuccess);
                    }
                    AsyncResult::AuthFailed(msg) => {
                        self.authenticating = false;
                        self.login.set_loading(false);
                        self.dispatch(AppAction::LoginFailed(msg));
                    }
                    AsyncResult::Courses(courses) => {
                        self.loading_courses = false;
                        self.dispatch(AppAction::CoursesLoaded(courses));
                    }
                    AsyncResult::CoursesFailed(msg) => {
                        self.loading_courses = false;
                        self.dispatch(AppAction::CoursesFailed(msg));
                    }
                    AsyncResult::Lectures(course_key, lectures) => {
                        self.dispatch(AppAction::LecturesLoaded(course_key, lectures));
                    }
                    AsyncResult::LecturesFailed(msg) => {
                        self.dispatch(AppAction::LecturesFailed(msg));
                    }
                    AsyncResult::Pages(lecture_key, pages) => {
                        self.dispatch(AppAction::PagesLoaded(lecture_key, pages));
                    }
                    AsyncResult::PagesFailed(msg) => {
                        self.dispatch(AppAction::PagesFailed(msg));
                    }
                }
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
            Screen::Selector => self.selector.render(frame, area),
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

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Block input during authentication
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

        // Screen-specific
        match self.screen {
            Screen::Login => {
                if let Some(action) = self
                    .login
                    .handle_event(Event::Key(event::KeyEvent::new(code, modifiers)))
                {
                    match action {
                        LoginAction::Submit(creds) => self.dispatch(AppAction::Login(creds)),
                        LoginAction::SwitchField => {
                            self.login.update(action);
                        }
                    }
                }
            }
            Screen::Selector => {
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
                        SelectorAction::SelectionChanged | SelectorAction::Confirm => {}
                    }
                }
            }
        }
    }

    fn dispatch(&mut self, action: AppAction) {
        match action {
            AppAction::Login(creds) => {
                self.authenticating = true;
                self.login.set_loading(true);
                self.error = None;
                self.start_authentication(creds);
            }
            AppAction::LoginSuccess => {
                self.error = None;
                self.screen = Screen::Selector;
                self.dispatch(AppAction::FetchCourses);
            }
            AppAction::LoginFailed(msg)
            | AppAction::ShowError(msg)
            | AppAction::CoursesFailed(msg)
            | AppAction::LecturesFailed(msg)
            | AppAction::PagesFailed(msg) => {
                self.error = Some(msg);
            }
            AppAction::FetchCourses => {
                self.loading_courses = true;
                self.start_fetch_courses();
            }
            AppAction::CoursesLoaded(courses) => {
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
            }
            AppAction::FetchPages(key) => {
                self.selector.set_loading_pages(true);
                self.start_fetch_pages(key);
            }
            AppAction::PagesLoaded(lecture_key, pages) => {
                self.selector.set_pages(pages, &lecture_key);
            }
            AppAction::Navigate(screen) => {
                self.screen = screen;
            }
            AppAction::ClearError => {
                self.error = None;
            }
            AppAction::Quit => {
                self.running = false;
            }
        }
    }

    fn start_authentication(&self, credentials: Credentials) {
        let collect = self.collect.clone();
        let tx = self.async_tx.clone();

        tokio::spawn(async move {
            let result = collect.authenticate(&credentials).await;
            let async_result = match result {
                Ok(()) => AsyncResult::AuthSuccess,
                Err(e) => {
                    let msg = match e {
                        collect::error::CollectError::Authentication { reason } => {
                            format!("認証に失敗しました: {reason}")
                        }
                        _ => "ログインに失敗しました。ユーザー名とパスワードを確認してください"
                            .to_string(),
                    };
                    AsyncResult::AuthFailed(msg)
                }
            };
            let _ = tx.send(async_result).await;
        });
    }

    fn start_fetch_courses(&self) {
        let collect = self.collect.clone();
        let tx = self.async_tx.clone();
        let year = self.year.and_then(|y| Year::new(y).ok());

        tokio::spawn(async move {
            let result = collect.get_courses(year).await;
            let async_result = match result {
                Ok(courses) => AsyncResult::Courses(courses),
                Err(_) => AsyncResult::CoursesFailed("科目の取得に失敗しました".to_string()),
            };
            let _ = tx.send(async_result).await;
        });
    }

    fn start_fetch_lectures(&self, course_key: CourseKey) {
        let collect = self.collect.clone();
        let tx = self.async_tx.clone();

        tokio::spawn(async move {
            let result = collect.get_lectures(&course_key).await;
            let async_result = match result {
                Ok(lectures) => AsyncResult::Lectures(course_key, lectures),
                Err(_) => AsyncResult::LecturesFailed("講義の取得に失敗しました".to_string()),
            };
            let _ = tx.send(async_result).await;
        });
    }

    fn start_fetch_pages(&self, lecture_key: LectureKey) {
        let collect = self.collect.clone();
        let tx = self.async_tx.clone();

        tokio::spawn(async move {
            let result = collect.get_pages(&lecture_key).await;
            let async_result = match result {
                Ok(pages) => AsyncResult::Pages(lecture_key, pages),
                Err(_) => AsyncResult::PagesFailed("ページの取得に失敗しました".to_string()),
            };
            let _ = tx.send(async_result).await;
        });
    }
}
