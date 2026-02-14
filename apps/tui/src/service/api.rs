use collect::{Collect, Course, CourseKey, Credentials, Lecture, LectureKey, LecturePage, Year};

use crate::error::TuiError;

#[derive(Clone)]
pub struct ApiClient {
    pub collect: Collect,
}

impl ApiClient {
    pub const fn new(collect: Collect) -> Self {
        Self { collect }
    }

    pub async fn authenticate(&self, credentials: &Credentials) -> Result<(), TuiError> {
        self.collect
            .authenticate(credentials)
            .await
            .map_err(|e| TuiError::from_collect(e, "認証"))
    }

    pub async fn get_courses(&self, year: Option<Year>) -> Result<Vec<Course>, TuiError> {
        self.collect
            .get_courses(year)
            .await
            .map_err(|e| TuiError::from_collect(e, "科目一覧"))
    }

    pub async fn get_lectures(&self, course_key: &CourseKey) -> Result<Vec<Lecture>, TuiError> {
        self.collect
            .get_lectures(course_key)
            .await
            .map_err(|e| TuiError::from_collect(e, "講義一覧"))
    }

    pub async fn get_archive_years(&self) -> Result<Vec<Year>, TuiError> {
        self.collect
            .get_archive_years()
            .await
            .map_err(|e| TuiError::from_collect(e, "年度一覧"))
    }

    pub async fn get_pages(&self, lecture_key: &LectureKey) -> Result<Vec<LecturePage>, TuiError> {
        self.collect
            .get_pages(lecture_key)
            .await
            .map_err(|e| TuiError::from_collect(e, "ページ一覧"))
    }
}
