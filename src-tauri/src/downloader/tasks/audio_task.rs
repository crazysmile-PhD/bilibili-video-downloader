use std::{collections::HashMap, sync::Arc};

use eyre::{WrapErr, eyre};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tokio::task::JoinSet;
use tracing::{Instrument, instrument};

use crate::{
    config::FileExistAction,
    downloader::{
        download_progress::DownloadProgress,
        download_task::DownloadTask,
        media_chunk::MediaChunk,
        media_download::{MediaKind, download_media},
    },
    extensions::AppHandleExt,
    types::{
        audio_quality::AudioQuality, bangumi_media_url::BangumiMediaUrl,
        cheese_media_url::CheeseMediaUrl, normal_media_url::NormalMediaUrl,
    },
    utils,
};

const CHUNK_SIZE: u64 = 2 * 1024 * 1024; // 2MB

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct AudioTask {
    pub selected: bool,
    pub url: String,
    pub audio_quality: AudioQuality,
    pub content_length: u64,
    pub chunks: Vec<MediaChunk>,
    pub completed: bool,
    pub skipped: bool,
}

impl AudioTask {
    pub async fn prepare_normal(&mut self, app: &AppHandle, media_url: &NormalMediaUrl) {
        let mut join_set = JoinSet::new();

        if let Some(medias) = &media_url.dash.audio {
            for media in medias {
                let app = app.clone();
                let id = media.id;

                let mut urls = Vec::new();
                urls.extend_from_slice(&media.backup_url);
                urls.push(media.base_url.clone());

                let get_url_with_content_length_task = async move {
                    let bili_client = app.get_bili_client();
                    let url_with_content_length =
                        bili_client.get_url_with_content_length(urls).await;
                    MediaForPrepare {
                        id,
                        url_with_content_length,
                    }
                };

                join_set.spawn(get_url_with_content_length_task.in_current_span());
            }
        }

        if let Some(medias) = &media_url.dash.dolby.audio {
            for media in medias {
                let app = app.clone();
                let id = media.id;

                let mut urls = Vec::new();
                urls.extend_from_slice(&media.backup_url);
                urls.push(media.base_url.clone());

                let get_url_with_content_length_task = async move {
                    let bili_client = app.get_bili_client();
                    let url_with_content_length =
                        bili_client.get_url_with_content_length(urls).await;
                    MediaForPrepare {
                        id,
                        url_with_content_length,
                    }
                };

                join_set.spawn(get_url_with_content_length_task.in_current_span());
            }
        }

        let flac = media_url.dash.flac.as_ref();
        if let Some(media) = flac.and_then(|flac| flac.audio.as_ref()) {
            let app = app.clone();
            let id = media.id;

            let mut urls = Vec::new();
            urls.extend_from_slice(&media.backup_url);
            urls.push(media.base_url.clone());

            let get_url_with_content_length_task = async move {
                let bili_client = app.get_bili_client();
                let url_with_content_length = bili_client.get_url_with_content_length(urls).await;
                MediaForPrepare {
                    id,
                    url_with_content_length,
                }
            };

            join_set.spawn(get_url_with_content_length_task.in_current_span());
        }

        let mut medias: Vec<MediaForPrepare> = Vec::new();

        while let Some(join_result) = join_set.join_next().await {
            let Ok(media) = join_result else {
                continue;
            };

            if !media.url_with_content_length.is_empty() {
                medias.push(media);
            }
        }

        self.prepare(app, &medias);
    }

    pub async fn prepare_bangumi(&mut self, app: &AppHandle, media_url: &BangumiMediaUrl) {
        let Some(dash) = &media_url.dash else {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        };

        let Some(medias) = &dash.audio else {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        };

        if medias.is_empty() {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        }

        let mut join_set = JoinSet::new();

        for media in medias {
            let app = app.clone();
            let id = media.id;

            let mut urls = Vec::new();
            urls.extend_from_slice(&media.backup_url);
            urls.push(media.base_url.clone());

            let get_url_with_content_length_task = async move {
                let bili_client = app.get_bili_client();
                let url_with_content_length = bili_client.get_url_with_content_length(urls).await;
                MediaForPrepare {
                    id,
                    url_with_content_length,
                }
            };

            join_set.spawn(get_url_with_content_length_task.in_current_span());
        }

        let mut medias: Vec<MediaForPrepare> = Vec::new();

        while let Some(join_result) = join_set.join_next().await {
            let Ok(media) = join_result else {
                continue;
            };

            if !media.url_with_content_length.is_empty() {
                medias.push(media);
            }
        }

        self.prepare(app, &medias);
    }

    pub async fn prepare_cheese(&mut self, app: &AppHandle, media_url: &CheeseMediaUrl) {
        let Some(dash) = &media_url.dash else {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        };

        let Some(medias) = &dash.audio else {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        };

        if medias.is_empty() {
            // 如果没有音频，则直接返回
            self.completed = true;
            return;
        }

        let mut join_set = JoinSet::new();

        for media in medias {
            let app = app.clone();
            let id = media.id;

            let mut urls = Vec::new();
            urls.extend_from_slice(&media.backup_url);
            urls.push(media.base_url.clone());

            let get_url_with_content_length_task = async move {
                let bili_client = app.get_bili_client();
                let url_with_content_length = bili_client.get_url_with_content_length(urls).await;
                MediaForPrepare {
                    id,
                    url_with_content_length,
                }
            };

            join_set.spawn(get_url_with_content_length_task.in_current_span());
        }

        let mut medias: Vec<MediaForPrepare> = Vec::new();

        while let Some(join_result) = join_set.join_next().await {
            let Ok(media) = join_result else {
                continue;
            };

            if !media.url_with_content_length.is_empty() {
                medias.push(media);
            }
        }

        self.prepare(app, &medias);
    }

    fn prepare(&mut self, app: &AppHandle, medias: &[MediaForPrepare]) {
        if medias.is_empty() {
            self.completed = true;
            return;
        }

        // 如果`audio_quality`为`Unknown`，则更倾向于使用优先级选择
        let prefer_select_by_priority = self.audio_quality == AudioQuality::Unknown;

        let selected_media = if prefer_select_by_priority {
            select_media_by_priority(app, medias)
        } else {
            select_exact_match_media(self, medias).or_else(|| select_media_by_priority(app, medias))
        };

        let Some(media) = selected_media else {
            self.completed = true;
            return;
        };

        self.audio_quality = media.id.into();

        let (url, content_length) = media
            .url_with_content_length
            .iter()
            .find(|(url, _)| url.starts_with("https://upos-"))
            .unwrap_or(&media.url_with_content_length[0])
            .clone();

        self.url = url;

        if self.content_length != content_length {
            let chunk_count = content_length.div_ceil(CHUNK_SIZE);

            #[allow(clippy::cast_possible_truncation)]
            let mut chunks = Vec::with_capacity(chunk_count as usize);
            for i in 0..chunk_count {
                let start = i * CHUNK_SIZE;
                let end = std::cmp::min(start + CHUNK_SIZE, content_length) - 1;
                chunks.push(MediaChunk {
                    start,
                    end,
                    completed: false,
                });
            }

            self.content_length = content_length;
            self.chunks = chunks;
        }
    }

    pub fn mark_uncompleted(&mut self) {
        self.completed = false;
        self.chunks.iter_mut().for_each(|chunk| {
            chunk.completed = false;
        });
        self.skipped = false;
    }

    pub fn is_completed(&self) -> bool {
        !self.selected || self.completed
    }

    #[allow(clippy::too_many_lines)]
    #[instrument(level = "error", skip_all)]
    pub async fn process(
        &self,
        download_task: &Arc<DownloadTask>,
        progress: &DownloadProgress,
    ) -> eyre::Result<()> {
        let (episode_dir, filename) = (&progress.episode_dir, &progress.filename);
        let audio_task = progress.audio_task.clone();

        let m4a_path = episode_dir.join(format!("{filename}.m4a"));
        let file_exist_action = download_task.app.get_config().read().file_exist_action;
        if file_exist_action == FileExistAction::Skip && m4a_path.exists() {
            tracing::debug!("音频文件已存在，跳过下载");
            download_task.update_progress(|p| {
                p.audio_task.skipped = true;
                p.audio_task.completed = true;
            });
            return Ok(());
        }

        let temp_file_path = episode_dir.join(format!(
            "{filename}.m4a.com.lanyeeee.bilibili-video-downloader"
        ));

        download_media(
            download_task,
            &temp_file_path,
            &audio_task.url,
            audio_task.content_length,
            &audio_task.chunks,
            MediaKind::Audio,
        )
        .await?;

        let is_audio_file_complete = utils::is_mp4_complete(&temp_file_path).wrap_err(format!(
            "检查音频文件`{}`是否完整失败",
            temp_file_path.display()
        ))?;

        if !is_audio_file_complete {
            download_task.update_progress(|p| p.audio_task.mark_uncompleted());
            return Err(eyre!(
                "音频文件`{}`不完整，[继续]会重新下载所有分片",
                temp_file_path.display()
            ));
        }

        // 重命名临时文件
        if m4a_path.exists() {
            std::fs::remove_file(&m4a_path)
                .wrap_err(format!("删除已存在的音频文件`{}`失败", m4a_path.display()))?;
        }
        std::fs::rename(&temp_file_path, &m4a_path).wrap_err(format!(
            "将临时文件`{}`重命名为`{}`失败",
            temp_file_path.display(),
            m4a_path.display()
        ))?;

        download_task.update_progress(|p| p.audio_task.completed = true);

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct MediaForPrepare {
    pub id: i64,
    pub url_with_content_length: Vec<(String, u64)>,
}

fn select_exact_match_media(
    audio_task: &AudioTask,
    medias: &[MediaForPrepare],
) -> Option<MediaForPrepare> {
    let media = medias.iter().find(|m| {
        let quality: AudioQuality = m.id.into();
        quality == audio_task.audio_quality
    });

    media.cloned()
}

fn select_media_by_priority(
    app: &AppHandle,
    medias: &[MediaForPrepare],
) -> Option<MediaForPrepare> {
    let quality_priority = app.get_config().read().audio_quality_priority.clone();

    let priority_map: HashMap<&AudioQuality, usize> = quality_priority
        .iter()
        .enumerate()
        .map(|(index, quality)| (quality, index))
        .collect();

    let media = medias.iter().min_by_key(|media| {
        let quality: AudioQuality = media.id.into();
        priority_map.get(&quality).unwrap_or(&usize::MAX)
    });

    media.cloned()
}
