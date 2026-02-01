use collect::{Collect, Credentials};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;

use crate::components::{login::LoginAction, Component, LoginComponent};
use crate::ui::{self, terminal, Theme, Tui};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Login,
    Main,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppAction {
    Login(Credentials),
    LoginSuccess,
    LoginFailed(String),
    Navigate(Screen),
    ShowError(String),
    ClearError,
    Quit,
}

#[derive(Debug)]
enum AuthResult {
    Success,
    Failed(String),
}

pub struct App {
    screen: Screen,
    error: Option<String>,
    running: bool,
    download_path: PathBuf,
    year: Option<u32>,
    login: LoginComponent,
    theme: Theme,
    collect: Collect,
    auth_tx: mpsc::Sender<AuthResult>,
    auth_rx: mpsc::Receiver<AuthResult>,
    authenticating: bool,
}

impl App {
    pub fn new(download_path: Option<PathBuf>, year: Option<u32>) -> Self {
        let (auth_tx, auth_rx) = mpsc::channel(1);
        let collect = Collect::default();

        Self {
            screen: Screen::default(),
            error: None,
            running: true,
            download_path: download_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            year,
            login: LoginComponent::new(),
            theme: Theme::default(),
            collect,
            auth_tx,
            auth_rx,
            authenticating: false,
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

            if let Ok(result) = self.auth_rx.try_recv() {
                self.authenticating = false;
                self.login.set_loading(false);
                match result {
                    AuthResult::Success => self.dispatch(AppAction::LoginSuccess),
                    AuthResult::Failed(msg) => self.dispatch(AppAction::LoginFailed(msg)),
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
            Screen::Main => self.render_main(frame, area),
        }

        if self.authenticating {
            ui::render_loading(frame, "ログイン中...", &self.theme);
        }

        if let Some(ref error) = self.error {
            ui::render_error(frame, error, &self.theme);
        }
    }

    fn render_main(&self, frame: &mut Frame, area: Rect) {
        let text = format!(
            "Path: {}\nYear: {}",
            self.download_path.display(),
            self.year.map_or("-".into(), |y| y.to_string())
        );
        let widget = Paragraph::new(text)
            .block(
                Block::default()
                    .title("moocs-collect")
                    .borders(Borders::ALL),
            )
            .style(self.theme.normal_style());
        frame.render_widget(widget, area);
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
            Screen::Main => {}
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
                self.screen = Screen::Main;
            }
            AppAction::LoginFailed(msg) | AppAction::ShowError(msg) => {
                self.error = Some(msg);
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
        let tx = self.auth_tx.clone();

        tokio::spawn(async move {
            let result = collect.authenticate(&credentials).await;
            let auth_result = match result {
                Ok(()) => AuthResult::Success,
                Err(e) => {
                    let msg = match e {
                        collect::error::CollectError::Authentication { reason } => {
                            format!("認証に失敗しました: {reason}")
                        }
                        _ => "ログインに失敗しました。ユーザー名とパスワードを確認してください"
                            .to_string(),
                    };
                    AuthResult::Failed(msg)
                }
            };
            // Ignore send error (receiver might be dropped if app is closing)
            let _ = tx.send(auth_result).await;
        });
    }
}
