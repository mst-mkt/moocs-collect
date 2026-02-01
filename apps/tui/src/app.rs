use collect::Credentials;
use color_eyre::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::{io, path::PathBuf, time::Duration};

use crate::components::{login::LoginAction, Component, LoginComponent};
use crate::ui::theme::Theme;

// Screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Login,
    Main,
}

// Action
#[derive(Debug, Clone)]
pub enum Action {
    Login(Credentials),
    ClearError,
    Quit,
}

// App
pub struct App {
    screen: Screen,
    error: Option<String>,
    running: bool,
    download_path: PathBuf,
    year: Option<u32>,
    login: LoginComponent,
    theme: Theme,
}

impl App {
    pub fn new(download_path: Option<PathBuf>, year: Option<u32>) -> Self {
        Self {
            screen: Screen::default(),
            error: None,
            running: true,
            download_path: download_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            year,
            login: LoginComponent::new(),
            theme: Theme::default(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = setup_terminal()?;
        let result = self.main_loop(&mut terminal);
        restore_terminal()?;
        result
    }

    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        while self.running {
            terminal.draw(|frame| self.render(frame))?;

            if event::poll(Duration::from_millis(100))? {
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

        if let Some(ref error) = self.error {
            crate::ui::error::render_error_popup(frame, error, &self.theme);
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
        // Global: Quit
        if matches!(code, KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL))
            || matches!(code, KeyCode::Esc | KeyCode::Char('q'))
        {
            if self.error.is_some() {
                self.dispatch(Action::ClearError);
            } else {
                self.dispatch(Action::Quit);
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
                        LoginAction::Submit(creds) => self.dispatch(Action::Login(creds)),
                        LoginAction::SwitchField => {
                            self.login.update(action);
                        }
                    }
                }
            }
            Screen::Main => {}
        }
    }

    fn dispatch(&mut self, action: Action) {
        match action {
            Action::Login(_creds) => {
                // TODO: Implement authentication
                self.screen = Screen::Main;
            }
            Action::ClearError => {
                self.error = None;
            }
            Action::Quit => {
                self.running = false;
            }
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
