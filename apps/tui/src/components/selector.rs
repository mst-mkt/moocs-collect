use collect::{Course, CourseKey, Lecture, LectureKey, LecturePage, PageKey};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};
use std::collections::{HashMap, HashSet};

use super::Component;
use crate::ui::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    #[default]
    Course,
    Lecture,
    Page,
}

#[derive(Debug, Clone)]
pub enum SelectorAction {
    FetchLectures(CourseKey),
    FetchPages(LectureKey),
    SelectionChanged,
    Confirm,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    pub courses: HashSet<CourseKey>,
    pub lectures: HashSet<LectureKey>,
    pub pages: HashSet<PageKey>,
}

impl SelectionState {
    pub fn is_course_selected(&self, key: &CourseKey) -> bool {
        self.courses.contains(key)
    }

    pub fn is_lecture_selected(&self, key: &LectureKey) -> bool {
        self.lectures.contains(key)
    }

    pub fn is_page_selected(&self, key: &PageKey) -> bool {
        self.pages.contains(key)
    }

    pub fn toggle_course(&mut self, key: CourseKey) -> bool {
        if self.courses.contains(&key) {
            self.courses.remove(&key);
            false
        } else {
            self.courses.insert(key);
            true
        }
    }

    pub fn toggle_lecture(&mut self, key: LectureKey) -> bool {
        if self.lectures.contains(&key) {
            self.lectures.remove(&key);
            false
        } else {
            self.lectures.insert(key);
            true
        }
    }

    pub fn toggle_page(&mut self, key: PageKey) -> bool {
        if self.pages.contains(&key) {
            self.pages.remove(&key);
            false
        } else {
            self.pages.insert(key);
            true
        }
    }

    pub fn select_all_lectures(&mut self, lectures: &[Lecture]) {
        for lecture in lectures {
            self.lectures.insert(lecture.key.clone());
        }
    }

    pub fn deselect_all_lectures(&mut self, lectures: &[Lecture]) {
        for lecture in lectures {
            self.lectures.remove(&lecture.key);
        }
    }

    pub fn select_all_pages(&mut self, pages: &[LecturePage]) {
        for page in pages {
            self.pages.insert(page.key.clone());
        }
    }

    pub fn deselect_all_pages(&mut self, pages: &[LecturePage]) {
        for page in pages {
            self.pages.remove(&page.key);
        }
    }
}

pub struct SelectorComponent {
    courses: Vec<Course>,
    lectures: Vec<Lecture>,
    pages: Vec<LecturePage>,

    lectures_cache: HashMap<CourseKey, Vec<Lecture>>,
    pages_cache: HashMap<LectureKey, Vec<LecturePage>>,

    course_state: ListState,
    lecture_state: ListState,
    page_state: ListState,

    selection: SelectionState,
    current_column: Column,

    loading_lectures: bool,
    loading_pages: bool,

    focused_course_key: Option<CourseKey>,
    focused_lecture_key: Option<LectureKey>,

    theme: Theme,
}

impl Component for SelectorComponent {
    type Action = SelectorAction;

    fn new() -> Self {
        Self {
            courses: Vec::new(),
            lectures: Vec::new(),
            pages: Vec::new(),
            lectures_cache: HashMap::new(),
            pages_cache: HashMap::new(),
            course_state: ListState::default(),
            lecture_state: ListState::default(),
            page_state: ListState::default(),
            selection: SelectionState::default(),
            current_column: Column::default(),
            loading_lectures: false,
            loading_pages: false,
            focused_course_key: None,
            focused_lecture_key: None,
            theme: Theme::default(),
        }
    }

    fn handle_event(&mut self, event: Event) -> Option<Self::Action> {
        if let Event::Key(key) = event {
            return self.handle_key_event(key);
        }
        None
    }

    fn update(&mut self, action: Self::Action) -> Option<Self::Action> {
        Some(action)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let [course_area, lecture_area, page_area] = Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .areas(area);

        self.render_course_list(frame, course_area);
        self.render_lecture_list(frame, lecture_area);
        self.render_page_list(frame, page_area);
    }
}

impl SelectorComponent {
    pub fn set_courses(&mut self, courses: Vec<Course>) {
        self.courses = courses;
        if !self.courses.is_empty() {
            self.course_state.select(Some(0));
            self.focused_course_key = self.courses.first().map(|c| c.key.clone());
        }
        self.lectures.clear();
        self.pages.clear();
        self.lecture_state.select(None);
        self.page_state.select(None);
    }

    pub fn set_lectures(&mut self, lectures: Vec<Lecture>, for_course: &CourseKey) {
        if self.focused_course_key.as_ref() != Some(for_course) {
            return;
        }
        self.loading_lectures = false;
        self.lectures_cache
            .insert(for_course.clone(), lectures.clone());
        self.lectures = lectures;

        if self.lectures.is_empty() {
            self.lecture_state.select(None);
            self.focused_lecture_key = None;
        } else {
            self.lecture_state.select(Some(0));
            self.focused_lecture_key = self.lectures.first().map(|l| l.key.clone());
        }

        if let Some(course_key) = &self.focused_course_key {
            if self.selection.is_course_selected(course_key) {
                self.selection.select_all_lectures(&self.lectures);
            }
        }

        self.pages.clear();
        self.page_state.select(None);
    }

    pub fn set_pages(&mut self, pages: Vec<LecturePage>, for_lecture: &LectureKey) {
        if self.focused_lecture_key.as_ref() != Some(for_lecture) {
            return;
        }
        self.loading_pages = false;
        self.pages_cache
            .insert(for_lecture.clone(), pages.clone());
        self.pages = pages;

        if self.pages.is_empty() {
            self.page_state.select(None);
        } else {
            self.page_state.select(Some(0));
        }

        if let Some(lecture_key) = &self.focused_lecture_key {
            if self.selection.is_lecture_selected(lecture_key) {
                self.selection.select_all_pages(&self.pages);
            }
        }
    }

    pub const fn set_loading_lectures(&mut self, loading: bool) {
        self.loading_lectures = loading;
    }

    pub const fn set_loading_pages(&mut self, loading: bool) {
        self.loading_pages = loading;
    }

    pub fn get_focused_lecture_key(&self) -> Option<LectureKey> {
        self.focused_lecture_key.clone()
    }

    pub fn try_load_pages_from_cache(&mut self) -> bool {
        if let Some(key) = &self.focused_lecture_key {
            if let Some(cached) = self.pages_cache.get(key) {
                self.pages = cached.clone();
                if self.pages.is_empty() {
                    self.page_state.select(None);
                } else {
                    self.page_state.select(Some(0));
                }
                return true;
            }
        }
        false
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<SelectorAction> {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_column_left();
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_column_right();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
            KeyCode::Char(' ') => self.toggle_selection(),
            KeyCode::Enter => Some(SelectorAction::Confirm),
            _ => None,
        }
    }

    fn move_column_left(&mut self) {
        self.current_column = match self.current_column {
            Column::Course | Column::Lecture => Column::Course,
            Column::Page => Column::Lecture,
        };
    }

    fn move_column_right(&mut self) {
        self.current_column = match self.current_column {
            Column::Course => {
                if !self.lectures.is_empty() || self.loading_lectures {
                    Column::Lecture
                } else {
                    Column::Course
                }
            }
            Column::Lecture => {
                if !self.pages.is_empty() || self.loading_pages {
                    Column::Page
                } else {
                    Column::Lecture
                }
            }
            Column::Page => Column::Page,
        };
    }

    fn move_selection_up(&mut self) -> Option<SelectorAction> {
        self.move_selection(-1)
    }

    fn move_selection_down(&mut self) -> Option<SelectorAction> {
        self.move_selection(1)
    }

    fn move_selection(&mut self, delta: i32) -> Option<SelectorAction> {
        match self.current_column {
            Column::Course => {
                Self::move_list_selection(&mut self.course_state, self.courses.len(), delta);
                self.on_course_focus_changed()
            }
            Column::Lecture => {
                Self::move_list_selection(&mut self.lecture_state, self.lectures.len(), delta);
                self.on_lecture_focus_changed()
            }
            Column::Page => {
                Self::move_list_selection(&mut self.page_state, self.pages.len(), delta);
                None
            }
        }
    }

    fn move_list_selection(state: &mut ListState, len: usize, delta: i32) {
        if len == 0 {
            return;
        }
        let current = state.selected().unwrap_or(0);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        let next = (current as i32 + delta).rem_euclid(len as i32) as usize;
        state.select(Some(next));
    }

    fn on_course_focus_changed(&mut self) -> Option<SelectorAction> {
        let new_key = self
            .course_state
            .selected()
            .and_then(|i| self.courses.get(i))
            .map(|c| c.key.clone());

        if new_key != self.focused_course_key {
            self.focused_course_key.clone_from(&new_key);
            self.pages.clear();
            self.page_state.select(None);
            self.focused_lecture_key = None;

            if let Some(key) = new_key {
                if let Some(cached) = self.lectures_cache.get(&key) {
                    self.lectures = cached.clone();
                    if self.lectures.is_empty() {
                        self.lecture_state.select(None);
                    } else {
                        self.lecture_state.select(Some(0));
                        self.focused_lecture_key = self.lectures.first().map(|l| l.key.clone());
                        if !self.try_load_pages_from_cache() {
                            if let Some(page_key) = self.focused_lecture_key.clone() {
                                self.loading_pages = true;
                                return Some(SelectorAction::FetchPages(page_key));
                            }
                        }
                    }
                    return None;
                }
                self.lectures.clear();
                self.lecture_state.select(None);
                self.loading_lectures = true;
                return Some(SelectorAction::FetchLectures(key));
            }
            self.lectures.clear();
            self.lecture_state.select(None);
        }
        None
    }

    fn on_lecture_focus_changed(&mut self) -> Option<SelectorAction> {
        let new_key = self
            .lecture_state
            .selected()
            .and_then(|i| self.lectures.get(i))
            .map(|l| l.key.clone());

        if new_key != self.focused_lecture_key {
            self.focused_lecture_key.clone_from(&new_key);

            if let Some(key) = new_key {
                if let Some(cached) = self.pages_cache.get(&key) {
                    self.pages = cached.clone();
                    if self.pages.is_empty() {
                        self.page_state.select(None);
                    } else {
                        self.page_state.select(Some(0));
                    }
                    return None;
                }
                self.pages.clear();
                self.page_state.select(None);
                self.loading_pages = true;
                return Some(SelectorAction::FetchPages(key));
            }
            self.pages.clear();
            self.page_state.select(None);
        }
        None
    }

    fn toggle_selection(&mut self) -> Option<SelectorAction> {
        match self.current_column {
            Column::Course => {
                if let Some(course) = self
                    .course_state
                    .selected()
                    .and_then(|i| self.courses.get(i))
                {
                    let key = course.key.clone();
                    let selected = self.selection.toggle_course(key);

                    if selected {
                        self.selection.select_all_lectures(&self.lectures);
                        self.selection.select_all_pages(&self.pages);
                    } else {
                        self.selection.deselect_all_lectures(&self.lectures);
                        self.selection.deselect_all_pages(&self.pages);
                    }
                    return Some(SelectorAction::SelectionChanged);
                }
            }
            Column::Lecture => {
                if let Some(lecture) = self
                    .lecture_state
                    .selected()
                    .and_then(|i| self.lectures.get(i))
                {
                    let key = lecture.key.clone();
                    let selected = self.selection.toggle_lecture(key);

                    if selected {
                        self.selection.select_all_pages(&self.pages);
                    } else {
                        self.selection.deselect_all_pages(&self.pages);
                    }
                    return Some(SelectorAction::SelectionChanged);
                }
            }
            Column::Page => {
                if let Some(page) = self.page_state.selected().and_then(|i| self.pages.get(i)) {
                    let key = page.key.clone();
                    self.selection.toggle_page(key);
                    return Some(SelectorAction::SelectionChanged);
                }
            }
        }
        None
    }

    fn render_course_list(&self, frame: &mut Frame, area: Rect) {
        let title = " 科目 ";
        let items: Vec<_> = self
            .courses
            .iter()
            .map(|c| (self.selection.is_course_selected(&c.key), c.display_name()))
            .collect();
        self.render_list(
            frame,
            area,
            title,
            &items,
            &self.course_state,
            self.current_column == Column::Course,
        );
    }

    fn render_lecture_list(&self, frame: &mut Frame, area: Rect) {
        let title = if self.loading_lectures {
            " 講義 (読み込み中...) "
        } else {
            " 講義 "
        };
        let items: Vec<_> = self
            .lectures
            .iter()
            .map(|l| (self.selection.is_lecture_selected(&l.key), l.display_name()))
            .collect();
        self.render_list(
            frame,
            area,
            title,
            &items,
            &self.lecture_state,
            self.current_column == Column::Lecture,
        );
    }

    fn render_page_list(&self, frame: &mut Frame, area: Rect) {
        let title = if self.loading_pages {
            " ページ (読み込み中...) "
        } else {
            " ページ "
        };
        let items: Vec<_> = self
            .pages
            .iter()
            .map(|p| (self.selection.is_page_selected(&p.key), p.display_name()))
            .collect();
        self.render_list(
            frame,
            area,
            title,
            &items,
            &self.page_state,
            self.current_column == Column::Page,
        );
    }

    fn render_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        items: &[(bool, &str)],
        state: &ListState,
        focused: bool,
    ) {
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|(selected, name)| {
                let checkbox = if *selected { "[x] " } else { "[ ] " };
                ListItem::new(Line::from(vec![Span::raw(checkbox), Span::raw(*name)]))
            })
            .collect();

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                self.theme.focused_border_style()
            } else {
                self.theme.border_style()
            });

        let list = List::new(list_items)
            .block(block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .fg(self.theme.primary),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut state.clone());
    }

    pub fn request_initial_data(&mut self) -> Option<SelectorAction> {
        if let Some(course) = self.courses.first() {
            self.loading_lectures = true;
            self.focused_course_key = Some(course.key.clone());
            return Some(SelectorAction::FetchLectures(course.key.clone()));
        }
        None
    }
}
