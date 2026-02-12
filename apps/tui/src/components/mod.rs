use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

pub mod login;
pub mod selector;

pub use login::LoginComponent;
pub use selector::SelectorComponent;

pub trait Component {
    type Action: Clone + std::fmt::Debug;

    fn new() -> Self;
    fn handle_event(&mut self, event: Event) -> Option<Self::Action>;
    fn update(&mut self, action: Self::Action) -> Option<Self::Action>;
    fn render(&self, frame: &mut Frame, area: Rect);
}
