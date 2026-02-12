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

use super::download::ResolvedPage;
use super::Component;
use crate::ui::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    #[default]
    Course,
    Lecture,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

impl CheckState {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Unchecked => "[ ] ",
            Self::Checked => "[x] ",
            Self::Indeterminate => "[-] ",
        }
    }

    /// Compute aggregate check state from child states.
    fn aggregate(children: impl Iterator<Item = Self>) -> Self {
        let mut all_checked = true;
        let mut any_selected = false;
        let mut has_children = false;

        for state in children {
            has_children = true;
            match state {
                Self::Checked => any_selected = true,
                Self::Indeterminate => {
                    any_selected = true;
                    all_checked = false;
                }
                Self::Unchecked => all_checked = false,
            }
        }

        if !has_children || !any_selected {
            Self::Unchecked
        } else if all_checked {
            Self::Checked
        } else {
            Self::Indeterminate
        }
    }
}

#[derive(Debug, Clone)]
pub enum SelectorAction {
    FetchLectures(CourseKey),
    FetchPages(LectureKey),
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

    pub fn select_all_pages(&mut self, pages: &[LecturePage]) {
        for page in pages {
            self.pages.insert(page.key.clone());
        }
    }

    /// Deselect a course and all its cached lectures/pages
    pub fn deselect_course_cascade(
        &mut self,
        course_key: &CourseKey,
        lectures_cache: &HashMap<CourseKey, Vec<Lecture>>,
        pages_cache: &HashMap<LectureKey, Vec<LecturePage>>,
    ) {
        self.courses.remove(course_key);
        if let Some(lectures) = lectures_cache.get(course_key) {
            for lecture in lectures {
                self.lectures.remove(&lecture.key);
                if let Some(pages) = pages_cache.get(&lecture.key) {
                    for page in pages {
                        self.pages.remove(&page.key);
                    }
                }
            }
        }
    }

    /// Deselect a lecture and all its cached pages
    pub fn deselect_lecture_cascade(
        &mut self,
        lecture_key: &LectureKey,
        pages_cache: &HashMap<LectureKey, Vec<LecturePage>>,
    ) {
        self.lectures.remove(lecture_key);
        if let Some(pages) = pages_cache.get(lecture_key) {
            for page in pages {
                self.pages.remove(&page.key);
            }
        }
    }

    /// Get the check state for a course based on its children
    pub fn get_course_check_state(
        &self,
        course_key: &CourseKey,
        lectures_cache: &HashMap<CourseKey, Vec<Lecture>>,
        pages_cache: &HashMap<LectureKey, Vec<LecturePage>>,
    ) -> CheckState {
        if self.courses.contains(course_key) {
            return CheckState::Checked;
        }

        let Some(lectures) = lectures_cache.get(course_key) else {
            return CheckState::Unchecked;
        };

        CheckState::aggregate(
            lectures
                .iter()
                .map(|l| self.get_lecture_check_state(&l.key, pages_cache)),
        )
    }

    /// Get the check state for a lecture based on its children
    pub fn get_lecture_check_state(
        &self,
        lecture_key: &LectureKey,
        pages_cache: &HashMap<LectureKey, Vec<LecturePage>>,
    ) -> CheckState {
        if self.lectures.contains(lecture_key) {
            return CheckState::Checked;
        }

        let Some(pages) = pages_cache.get(lecture_key) else {
            return CheckState::Unchecked;
        };

        CheckState::aggregate(pages.iter().map(|p| {
            if self.pages.contains(&p.key) {
                CheckState::Checked
            } else {
                CheckState::Unchecked
            }
        }))
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
        self.lectures_cache
            .insert(for_course.clone(), lectures.clone());

        if self.focused_course_key.as_ref() != Some(for_course) {
            return;
        }

        self.loading_lectures = false;
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
        self.pages_cache.insert(for_lecture.clone(), pages.clone());

        if self.focused_lecture_key.as_ref() != Some(for_lecture) {
            return;
        }

        self.loading_pages = false;
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

    fn try_load_lectures_from_cache(&mut self) -> bool {
        let Some(key) = self.focused_course_key.clone() else {
            return false;
        };
        let Some(cached) = self.lectures_cache.get(&key).cloned() else {
            return false;
        };

        self.loading_lectures = false;
        self.lectures = cached;

        if self.selection.is_course_selected(&key) {
            self.selection.select_all_lectures(&self.lectures);
        }

        if self.lectures.is_empty() {
            self.lecture_state.select(None);
        } else {
            self.lecture_state.select(Some(0));
            self.focused_lecture_key = self.lectures.first().map(|l| l.key.clone());
        }
        true
    }

    pub fn try_load_pages_from_cache(&mut self) -> bool {
        let Some(key) = self.focused_lecture_key.clone() else {
            return false;
        };
        let Some(cached) = self.pages_cache.get(&key).cloned() else {
            return false;
        };

        self.loading_pages = false;
        self.pages = cached;

        let should_select = self.selection.is_lecture_selected(&key)
            || self.selection.is_course_selected(&key.course_key);
        if should_select {
            self.selection.select_all_pages(&self.pages);
        }

        if self.pages.is_empty() {
            self.page_state.select(None);
        } else {
            self.page_state.select(Some(0));
        }
        true
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
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
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
        let next = if delta > 0 {
            current.saturating_add(delta as usize).min(len - 1)
        } else {
            current.saturating_sub(delta.unsigned_abs() as usize)
        };
        state.select(Some(next));
    }

    fn on_course_focus_changed(&mut self) -> Option<SelectorAction> {
        let new_key = self
            .course_state
            .selected()
            .and_then(|i| self.courses.get(i))
            .map(|c| c.key.clone());

        if new_key == self.focused_course_key {
            return None;
        }

        self.focused_course_key.clone_from(&new_key);
        self.pages.clear();
        self.page_state.select(None);
        self.focused_lecture_key = None;

        let Some(key) = new_key else {
            self.lectures.clear();
            self.lecture_state.select(None);
            return None;
        };

        if self.try_load_lectures_from_cache() {
            if !self.try_load_pages_from_cache() {
                if let Some(lecture_key) = self.focused_lecture_key.clone() {
                    self.loading_pages = true;
                    return Some(SelectorAction::FetchPages(lecture_key));
                }
            }
            return None;
        }

        self.lectures.clear();
        self.lecture_state.select(None);
        self.loading_lectures = true;
        Some(SelectorAction::FetchLectures(key))
    }

    fn on_lecture_focus_changed(&mut self) -> Option<SelectorAction> {
        let new_key = self
            .lecture_state
            .selected()
            .and_then(|i| self.lectures.get(i))
            .map(|l| l.key.clone());

        if new_key == self.focused_lecture_key {
            return None;
        }

        self.focused_lecture_key.clone_from(&new_key);

        let Some(key) = new_key else {
            self.pages.clear();
            self.page_state.select(None);
            return None;
        };

        if self.try_load_pages_from_cache() {
            return None;
        }

        self.pages.clear();
        self.page_state.select(None);
        self.loading_pages = true;
        Some(SelectorAction::FetchPages(key))
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
                    let current_state = self.selection.get_course_check_state(
                        &key,
                        &self.lectures_cache,
                        &self.pages_cache,
                    );

                    match current_state {
                        CheckState::Checked => {
                            // Deselect course and all children
                            self.selection.deselect_course_cascade(
                                &key,
                                &self.lectures_cache,
                                &self.pages_cache,
                            );
                        }
                        CheckState::Unchecked | CheckState::Indeterminate => {
                            // Select course and all visible children
                            self.selection.courses.insert(key);
                            self.selection.select_all_lectures(&self.lectures);
                            self.selection.select_all_pages(&self.pages);
                        }
                    }
                    return None;
                }
            }
            Column::Lecture => {
                if let Some(lecture) = self
                    .lecture_state
                    .selected()
                    .and_then(|i| self.lectures.get(i))
                {
                    let key = lecture.key.clone();
                    let current_state = self
                        .selection
                        .get_lecture_check_state(&key, &self.pages_cache);

                    // Remove parent course from selection (becomes indeterminate)
                    self.selection.courses.remove(&key.course_key);

                    match current_state {
                        CheckState::Checked => {
                            // Deselect lecture and all children
                            self.selection
                                .deselect_lecture_cascade(&key, &self.pages_cache);
                        }
                        CheckState::Unchecked | CheckState::Indeterminate => {
                            // Select lecture and all visible pages
                            self.selection.lectures.insert(key);
                            self.selection.select_all_pages(&self.pages);
                        }
                    }
                    return None;
                }
            }
            Column::Page => {
                if let Some(page) = self.page_state.selected().and_then(|i| self.pages.get(i)) {
                    let key = page.key.clone();

                    // Remove parent lecture and course from selection (becomes indeterminate)
                    self.selection.lectures.remove(&key.lecture_key);
                    self.selection.courses.remove(&key.lecture_key.course_key);

                    self.selection.toggle_page(key);
                    return None;
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
            .map(|c| {
                let state = self.selection.get_course_check_state(
                    &c.key,
                    &self.lectures_cache,
                    &self.pages_cache,
                );
                (state, c.display_name())
            })
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
            .map(|l| {
                let state = self
                    .selection
                    .get_lecture_check_state(&l.key, &self.pages_cache);
                (state, l.display_name())
            })
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
            .map(|p| {
                let state = if self.selection.is_page_selected(&p.key) {
                    CheckState::Checked
                } else {
                    CheckState::Unchecked
                };
                (state, p.display_name())
            })
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
        items: &[(CheckState, &str)],
        state: &ListState,
        focused: bool,
    ) {
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|(check_state, name)| {
                let checkbox = check_state.symbol();
                ListItem::new(Line::from(vec![Span::raw(checkbox), Span::raw(*name)]))
            })
            .collect();

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if focused {
                self.theme.focused_border_style()
            } else {
                self.theme.inactive_style()
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

    /// Returns (courses needing lectures, lectures needing pages) that are
    /// selected but not yet cached.
    pub fn get_unfetched_for_selection(&self) -> (Vec<CourseKey>, Vec<LectureKey>) {
        let mut courses_to_fetch = Vec::new();
        let mut lectures_to_fetch = Vec::new();

        // Selected courses whose lectures haven't been fetched
        for course_key in &self.selection.courses {
            if let Some(lectures) = self.lectures_cache.get(course_key) {
                // Lectures cached — check each for pages
                for lecture in lectures {
                    if !self.pages_cache.contains_key(&lecture.key) {
                        lectures_to_fetch.push(lecture.key.clone());
                    }
                }
            } else {
                courses_to_fetch.push(course_key.clone());
            }
        }

        // Individually selected lectures (not under a selected course) whose pages haven't been fetched
        for lecture_key in &self.selection.lectures {
            if self.selection.courses.contains(&lecture_key.course_key) {
                continue; // Already handled above
            }
            if !self.pages_cache.contains_key(lecture_key) {
                lectures_to_fetch.push(lecture_key.clone());
            }
        }

        (courses_to_fetch, lectures_to_fetch)
    }

    /// Resolves all pages that should be downloaded based on the full selection
    /// hierarchy: selected courses → all lectures → all pages, selected
    /// lectures → all pages, and individually selected pages.
    pub fn resolve_selected_pages(&self) -> Vec<ResolvedPage> {
        let course_names: HashMap<&CourseKey, &str> = self
            .courses
            .iter()
            .map(|c| (&c.key, c.display_name()))
            .collect();

        let lecture_names: HashMap<&LectureKey, &str> = self
            .lectures_cache
            .values()
            .flat_map(|lectures| lectures.iter())
            .map(|l| (&l.key, l.display_name()))
            .collect();

        let page_names: HashMap<&PageKey, &str> = self
            .pages_cache
            .values()
            .flat_map(|pages| pages.iter())
            .map(|p| (&p.key, p.display_name()))
            .collect();

        let mut seen = HashSet::new();
        let mut resolved = Vec::new();

        let mut add_page = |page_key: &PageKey| {
            if !seen.insert(page_key.clone()) {
                return;
            }
            let lecture_key = &page_key.lecture_key;
            let course_key = &lecture_key.course_key;

            resolved.push(ResolvedPage {
                page_key: page_key.clone(),
                course_name: course_names
                    .get(course_key)
                    .copied()
                    .unwrap_or_else(|| course_key.slug.value())
                    .to_string(),
                lecture_name: lecture_names
                    .get(lecture_key)
                    .copied()
                    .unwrap_or_else(|| lecture_key.slug.value())
                    .to_string(),
                page_name: page_names
                    .get(page_key)
                    .copied()
                    .unwrap_or_else(|| page_key.slug.value())
                    .to_string(),
            });
        };

        // 1. Pages from selected courses (via cache)
        for course_key in &self.selection.courses {
            if let Some(lectures) = self.lectures_cache.get(course_key) {
                for lecture in lectures {
                    if let Some(pages) = self.pages_cache.get(&lecture.key) {
                        for page in pages {
                            add_page(&page.key);
                        }
                    }
                }
            }
        }

        // 2. Pages from individually selected lectures (not under selected course)
        for lecture_key in &self.selection.lectures {
            if self.selection.courses.contains(&lecture_key.course_key) {
                continue;
            }
            if let Some(pages) = self.pages_cache.get(lecture_key) {
                for page in pages {
                    add_page(&page.key);
                }
            }
        }

        // 3. Individually selected pages
        for page_key in &self.selection.pages {
            if self.selection.lectures.contains(&page_key.lecture_key)
                || self
                    .selection
                    .courses
                    .contains(&page_key.lecture_key.course_key)
            {
                continue;
            }
            add_page(page_key);
        }

        resolved
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
