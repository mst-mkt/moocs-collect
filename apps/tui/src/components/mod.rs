use crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

pub mod login;

pub use login::LoginComponent;

/// Common trait for all TUI components
pub trait Component {
    type Action: Clone + std::fmt::Debug;

    /// Create a new instance of the component
    fn new() -> Self;

    /// Handle input events
    fn handle_event(&mut self, event: Event) -> Option<Self::Action>;

    /// Update component state based on action
    fn update(&mut self, action: Self::Action) -> Option<Self::Action>;

    /// Render the component
    fn render(&self, frame: &mut Frame, area: Rect);
}
