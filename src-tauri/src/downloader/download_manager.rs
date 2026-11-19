use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use eyre::{WrapErr, eyre};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Semaphore;
use tracing::instrument;

use crate::{
    events::DownloadEvent,
    extensions::{AppHandleExt, EyreReportToMessage},
    types::{
        create_download_task_params::CreateDownloadTaskParams,
        restart_download_task_params::RestartDownloadTaskParams,
    },
};

use super::{
    download_task::{DownloadTask, RestoredDownloadTask},
    progress_store::{ProgressStore, read_progresses},
};

pub struct DownloadManager {
    pub app: AppHandle,
    pub progress_store: ProgressStore,
    pub task_sem: Arc<Semaphore>,
    pub media_chunk_sem: Arc<Semaphore>,
    pub byte_per_sec: Arc<AtomicU64>,
    pub download_tasks: RwLock<HashMap<String, Arc<DownloadTask>>>,
}

impl DownloadManager {
    pub fn new(app: AppHandle) -> eyre::Result<Self> {
        let (task_concurrency, chunk_concurrency) = {
            let config = app.get_config().inner().read();
            (config.task_concurrency, config.chunk_concurrency)
        };

        let progress_store = ProgressStore::new(app.path().app_data_dir()?.join(".下载任务"))?;
        let manager = Self {
            app,
            progress_store,
            task_sem: Arc::new(Semaphore::new(task_concurrency)),
            media_chunk_sem: Arc::new(Semaphore::new(chunk_concurrency)),
            byte_per_sec: Arc::new(AtomicU64::new(0)),
            download_tasks: RwLock::new(HashMap::new()),
        };

        tauri::async_runtime::spawn(Self::emit_download_speed_loop(
            manager.app.clone(),
            manager.byte_per_sec.clone(),
        ));

        Ok(manager)
    }

    #[instrument(level = "error", skip_all)]
    pub async fn restore_download_tasks(&self) -> eyre::Result<Vec<RestoredDownloadTask>> {
        let directory = self.progress_store.directory().to_owned();
        let progresses = tokio::task::spawn_blocking(move || read_progresses(&directory))
            .await
            .wrap_err("读取下载任务工作异常退出")??;
        let mut tasks = self.download_tasks.write();
        for progress in progresses {
            // Repeated restoration must not replace an active task with an older
            // disk snapshot. Recheck files removed while the read was running.
            if tasks.contains_key(&progress.task_id) {
                continue;
            }
            if !self.progress_store.contains(&progress.task_id)? {
                continue;
            }
            let task = DownloadTask::from_progress(self.app.clone(), progress);
            tasks.insert(task.task_id.clone(), task);
        }
        Ok(tasks
            .values()
            .map(|task| RestoredDownloadTask {
                state: *task.state_sender.borrow(),
                progress: task.progress.read().clone(),
            })
            .collect())
    }

    pub fn create_download_tasks(&self, params: &CreateDownloadTaskParams) {
        // Restoration must not discover a new checkpoint before registration and
        // create a second live task for the same ID.
        let mut tasks = self.download_tasks.write();
        let new_tasks = DownloadTask::from_params(&self.app, params);
        for new_task in new_tasks {
            tasks.insert(new_task.task_id.clone(), new_task);
        }
    }

    #[instrument(level = "error", skip_all)]
    pub fn pause_download_tasks(&self, task_ids: &Vec<String>) {
        let tasks = self.download_tasks.read();
        for task_id in task_ids {
            let span = tracing::error_span!("pause_download_task", task_id = task_id);
            let _enter = span.enter();

            let Some(task) = tasks.get(task_id) else {
                let err = eyre!("未找到ID对应的下载任务");
                let err_title = "暂停下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            };
            task.pause();
        }
    }

    #[instrument(level = "error", skip_all)]
    pub fn resume_download_tasks(&self, task_ids: &Vec<String>) {
        let tasks = self.download_tasks.read();
        for task_id in task_ids {
            let span = tracing::error_span!("resume_download_task", task_id = task_id);
            let _enter = span.enter();

            let Some(task) = tasks.get(task_id) else {
                let err = eyre!("未找到ID对应的下载任务");
                let err_title = "继续下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            };
            task.resume();
        }
    }

    #[instrument(level = "error", skip_all)]
    pub fn delete_download_tasks(&self, task_ids: &Vec<String>) {
        let mut tasks = self.download_tasks.write();
        for task_id in task_ids {
            let span = tracing::error_span!("delete_download_task", task_id = task_id);
            let _enter = span.enter();

            let Some(task) = tasks.remove(task_id) else {
                let err = eyre!("未找到ID对应的下载任务");
                let err_title = "删除下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            };
            if let Err(err) = task.delete() {
                let err_title = "删除下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                tasks.insert(task_id.clone(), task);
                continue;
            }
        }
    }

    #[instrument(level = "error", skip_all)]
    pub fn restart_download_tasks(&self, task_ids: &Vec<String>) {
        let tasks = self.download_tasks.read();
        for task_id in task_ids {
            let span = tracing::error_span!("restart_download_task", task_id = task_id);
            let _enter = span.enter();

            let Some(task) = tasks.get(task_id) else {
                let err = eyre!("未找到ID对应的下载任务");
                let err_title = "重来下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            };

            if let Err(err) = task.restart() {
                let err_title = "重来下载任务失败";
                let message = err.to_message();
                tracing::error!(err_title, message);
                continue;
            }
        }
    }

    #[instrument(level = "error", skip_all, fields(task_id = params.task_id))]
    pub fn restart_download_task(&self, params: &RestartDownloadTaskParams) {
        let task_id = &params.task_id;

        let tasks = self.download_tasks.read();
        let Some(task) = tasks.get(task_id) else {
            let err = eyre!("未找到ID对应的下载任务");
            let err_title = "重来下载任务失败";
            let message = err.to_message();
            tracing::error!(err_title, message);
            return;
        };

        if let Err(err) = task.restart_with_params(params) {
            let err_title = "重来下载任务失败";
            let message = err.to_message();
            tracing::error!(err_title, message);
        }
    }

    async fn emit_download_speed_loop(app: AppHandle, byte_per_sec: Arc<AtomicU64>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;
            let byte_per_sec = byte_per_sec.swap(0, Ordering::Relaxed);
            #[allow(clippy::cast_precision_loss)]
            let mega_byte_per_sec = byte_per_sec as f64 / 1024.0 / 1024.0;
            let speed = format!("{mega_byte_per_sec:.2}MB/s");
            let _ = DownloadEvent::Speed { speed }.emit(&app);
        }
    }
}
