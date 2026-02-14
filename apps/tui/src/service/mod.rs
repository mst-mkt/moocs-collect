mod api;
mod task;

use api::ApiClient;
use task::TaskManager;

use collect::{Collect, CourseKey, Credentials, LectureKey, PageKey, Year};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, Semaphore};

use crate::app::AppAction;

pub struct AppService {
    api: ApiClient,
    task: TaskManager,

    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
    download_path: PathBuf,

    year: Option<u32>,
}

impl AppService {
    pub const fn new(
        collect: Collect,
        client: reqwest::Client,
        semaphore: Arc<Semaphore>,
        action_tx: mpsc::Sender<AppAction>,
        download_path: PathBuf,
        year: Option<u32>,
    ) -> Self {
        Self {
            api: ApiClient::new(collect),
            task: TaskManager::new(action_tx),
            client,
            semaphore,
            download_path,
            year,
        }
    }

    pub fn download_path(&self) -> &std::path::Path {
        &self.download_path
    }

    pub const fn year(&self) -> Option<u32> {
        self.year
    }

    pub fn authenticate(&self, credentials: Credentials) {
        let api = self.api.clone();
        self.task.spawn(async move {
            match api.authenticate(&credentials).await {
                Ok(()) => AppAction::LoginSuccess,
                Err(e) => AppAction::Error(e),
            }
        });
    }

    pub const fn set_year(&mut self, year: Option<u32>) {
        self.year = year;
    }

    pub fn set_download_path(&mut self, path: PathBuf) {
        self.download_path = path;
    }

    pub fn fetch_archive_years(&self) {
        let api = self.api.clone();
        self.task.spawn(async move {
            match api.get_archive_years().await {
                Ok(years) => AppAction::ArchiveYearsLoaded(years),
                Err(e) => AppAction::Error(e),
            }
        });
    }

    pub fn fetch_courses(&self) {
        let api = self.api.clone();
        let year = self.year.and_then(|y| Year::new(y).ok());

        self.task.spawn(async move {
            match api.get_courses(year).await {
                Ok(courses) => AppAction::CoursesLoaded(courses),
                Err(e) => AppAction::Error(e),
            }
        });
    }

    pub fn fetch_lectures(&self, course_key: CourseKey) {
        let api = self.api.clone();

        self.task.spawn(async move {
            match api.get_lectures(&course_key).await {
                Ok(lectures) => AppAction::LecturesLoaded(course_key, lectures),
                Err(e) => AppAction::Error(e),
            }
        });
    }

    pub fn fetch_pages(&self, lecture_key: LectureKey) {
        let api = self.api.clone();

        self.task.spawn(async move {
            match api.get_pages(&lecture_key).await {
                Ok(pages) => AppAction::PagesLoaded(lecture_key, pages),
                Err(e) => AppAction::Error(e),
            }
        });
    }

    pub fn download(&self, page_key: PageKey) {
        let collect = self.api.collect.clone();
        let client = self.client.clone();
        let semaphore = self.semaphore.clone();
        let download_path = self.download_path.clone();
        let task = self.task.clone();

        self.task.spawn(async move {
            let (dl_tx, mut dl_rx) = mpsc::channel(16);

            let download_task = async {
                let result = crate::download::download_page(
                    &collect,
                    &client,
                    &page_key,
                    &download_path,
                    &dl_tx,
                    &semaphore,
                )
                .await;
                drop(dl_tx);
                result
            };

            let pk = page_key.clone();
            let forward_task = async move {
                while let Some(event) = dl_rx.recv().await {
                    let action = match event {
                        crate::download::DownloadEvent::Progress(key, pct, phase) => {
                            AppAction::DownloadProgress(key, pct, phase)
                        }
                    };
                    task.send(action).await;
                }
            };

            let (result, ()) = tokio::join!(download_task, forward_task);

            match result {
                Ok(()) => AppAction::DownloadCompleted(pk),
                Err(e) => AppAction::DownloadFailed(pk, format!("{e:#}")),
            }
        });
    }
}
