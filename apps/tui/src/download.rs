use collect::{pdf, Collect, PageKey, SlideContent};
use futures::stream::{self, StreamExt};
use rayon::prelude::*;
use std::{fs::create_dir_all, path::Path, sync::Arc, time::Duration};
use tokio::sync::{mpsc, Semaphore};

use crate::app::AppAction;

const MAX_RETRIES: u32 = 3;
const MAX_CONCURRENT_REQUESTS: usize = 4;

fn sanitize_filename(s: &str) -> String {
    #[cfg(windows)]
    const INVALID_CHARS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    #[cfg(unix)]
    const INVALID_CHARS: [char; 2] = ['/', '\0'];

    s.chars()
        .map(|c| if INVALID_CHARS.contains(&c) { '_' } else { c })
        .collect()
}

struct PageInfo {
    course_name: String,
    lecture_group: String,
    lecture_name: String,
    page_slug: String,
    page_title: String,
}

async fn get_page_info(collect: &Collect, page_key: &PageKey) -> anyhow::Result<PageInfo> {
    let page = collect.get_page_info(page_key).await?;
    let lecture = collect.get_lecture_info(&page.key.lecture_key).await?;
    let course = collect.get_course_info(&lecture.key.course_key).await?;

    let lecture_group = collect
        .get_lecture_groups(&lecture.key.course_key)
        .await
        .ok()
        .and_then(|groups| {
            groups
                .iter()
                .find(|group| group.course_key == lecture.key.course_key)
                .map(|group| group.display_name().to_string())
        })
        .unwrap_or_else(|| lecture.display_name().to_string());

    Ok(PageInfo {
        course_name: course.display_name().to_string(),
        lecture_group,
        lecture_name: lecture.display_name().to_string(),
        page_slug: page.key.slug.value().to_string(),
        page_title: page.display_name().to_string(),
    })
}

fn slide_dir_from_info(info: &PageInfo) -> String {
    format!(
        "{}/{} - {}",
        sanitize_filename(&info.course_name),
        sanitize_filename(&info.lecture_group),
        sanitize_filename(&info.lecture_name)
    )
}

async fn save_slides(
    collect: &Collect,
    client: &reqwest::Client,
    slide_contents: &[SlideContent],
    path: &Path,
    page_key: &PageKey,
    tx: &mpsc::Sender<AppAction>,
) -> anyhow::Result<()> {
    if slide_contents.is_empty() {
        return Ok(());
    }

    send_progress(tx, page_key, 60, "前処理中...").await;

    let page_info = get_page_info(collect, &slide_contents[0].page_key).await?;
    let dir = slide_dir_from_info(&page_info);
    let path = path.join(&dir);
    create_dir_all(&path)?;

    let preprocessor = pdf::PreProcessor::default();
    let total = slide_contents.len();

    let mut preprocessed = Vec::with_capacity(total);
    for (i, content) in slide_contents.iter().enumerate() {
        let processed = preprocessor.preprocess(client, content).await?;
        preprocessed.push(processed);

        #[allow(clippy::cast_possible_truncation)]
        let pct = 60 + ((i + 1) * 20 / total) as u8;
        send_progress(
            tx,
            page_key,
            pct,
            &format!("前処理中... ({}/{})", i + 1, total),
        )
        .await;
    }

    send_progress(tx, page_key, 85, "PDF変換中...").await;

    let path_clone = path.clone();
    let page_slug = page_info.page_slug.clone();
    let page_title = page_info.page_title.clone();
    let len = preprocessed.len();

    tokio::task::spawn_blocking(move || {
        preprocessed
            .par_iter()
            .enumerate()
            .try_for_each(|(i, content)| -> anyhow::Result<()> {
                let filename = match len {
                    1 => format!(
                        "{} - {}.pdf",
                        sanitize_filename(&page_slug),
                        sanitize_filename(&page_title)
                    ),
                    _ => format!(
                        "{} - {} ({}).pdf",
                        sanitize_filename(&page_slug),
                        sanitize_filename(&page_title),
                        i + 1
                    ),
                };

                let mut document = pdf::convert(content)?;
                let file_path = path_clone.join(&filename);
                document.save(&file_path)?;
                Ok(())
            })
    })
    .await??;

    send_progress(tx, page_key, 95, "保存完了").await;

    Ok(())
}

pub async fn download_page(
    collect: &Collect,
    client: &reqwest::Client,
    page_key: &PageKey,
    download_path: &Path,
    tx: &mpsc::Sender<AppAction>,
    semaphore: &Arc<Semaphore>,
) -> anyhow::Result<()> {
    send_progress(tx, page_key, 5, "スライド取得中...").await;

    let slides = collect.get_slides(page_key).await?;
    if slides.is_empty() {
        return Ok(());
    }

    send_progress(
        tx,
        page_key,
        10,
        &format!("コンテンツ取得中... (0/{})", slides.len()),
    )
    .await;

    let total_slides = slides.len();
    let tx_clone = tx.clone();
    let pk = page_key.clone();

    let mut indexed_contents: Vec<(usize, _)> = Vec::with_capacity(total_slides);
    let mut slide_stream = stream::iter(slides.into_iter().enumerate().map(|(i, slide)| {
        let collect = collect.clone();
        let semaphore = semaphore.clone();
        async move {
            let mut last_err = None;
            for attempt in 0..MAX_RETRIES {
                if attempt > 0 {
                    tokio::time::sleep(Duration::from_millis(1000 * u64::from(attempt))).await;
                }
                let _permit = semaphore.acquire().await.ok();
                match collect.get_slide_content(&slide).await {
                    Ok(content) => return (i, Ok(content)),
                    Err(e) => last_err = Some(e),
                }
            }
            (
                i,
                Err(last_err.unwrap_or_else(|| {
                    collect::error::CollectError::network(
                        "Failed to fetch slide content after retries",
                        None::<reqwest::Error>,
                    )
                })),
            )
        }
    }))
    .buffer_unordered(MAX_CONCURRENT_REQUESTS);

    let mut completed = 0usize;
    while let Some((i, result)) = slide_stream.next().await {
        indexed_contents.push((i, result?));
        completed += 1;
        #[allow(clippy::cast_possible_truncation)]
        let pct = 10 + (completed * 50 / total_slides) as u8;
        let _ = tx_clone
            .send(AppAction::DownloadProgress(
                pk.clone(),
                pct,
                format!("コンテンツ取得中... ({completed}/{})", total_slides),
            ))
            .await;
    }

    indexed_contents.sort_by_key(|(i, _)| *i);
    let contents: Vec<_> = indexed_contents.into_iter().map(|(_, c)| c).collect();

    save_slides(collect, client, &contents, download_path, page_key, tx).await?;

    Ok(())
}

async fn send_progress(tx: &mpsc::Sender<AppAction>, page_key: &PageKey, percent: u8, phase: &str) {
    let _ = tx
        .send(AppAction::DownloadProgress(
            page_key.clone(),
            percent,
            phase.to_string(),
        ))
        .await;
}
