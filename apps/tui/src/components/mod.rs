use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

pub mod download;
pub mod login;
pub mod selector;

pub use download::DownloadComponent;
pub use login::LoginComponent;
pub use selector::SelectorComponent;

pub trait Component {
    type Action: Clone + std::fmt::Debug;

    fn new() -> Self;
    fn handle_event(&mut self, event: Event) -> Option<Self::Action>;
    fn render(&self, frame: &mut Frame, area: Rect);
}
