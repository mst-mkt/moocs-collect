use crossterm::event::Event;
use ratatui::{layout::Rect, widgets::ListState, Frame};

use crate::ui::Theme;

pub mod download;
pub mod login;
pub mod selector;

pub use download::DownloadComponent;
pub use login::LoginComponent;
pub use selector::SelectorComponent;

pub trait Component {
    type Action;

    fn handle_event(&mut self, event: Event) -> Option<Self::Action>;
    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme);
}

pub fn move_list_selection(state: &mut ListState, len: usize, delta: i32) {
    if len == 0 {
        return;
    }
    let current = state.selected().unwrap_or(0);
    let abs = delta.unsigned_abs() as usize;
    let next = if delta > 0 {
        current.saturating_add(abs).min(len - 1)
    } else {
        current.saturating_sub(abs)
    };
    state.select(Some(next));
}
