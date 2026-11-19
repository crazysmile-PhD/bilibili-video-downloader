use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::storage::write_json_atomic as replace_json;
use eyre::{WrapErr, ensure, eyre};
use parking_lot::Mutex;

use super::download_progress::DownloadProgress;

/// One ordered path for checkpoint writes and deletion, independent of Tauri.
pub struct ProgressStore {
    directory: PathBuf,
    io: Mutex<()>,
}

impl ProgressStore {
    pub fn new(directory: PathBuf) -> eyre::Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            io: Mutex::new(()),
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn create(&self, progress: &DownloadProgress) -> eyre::Result<()> {
        let _io = self.io.lock();
        let path = progress_path(self.directory(), &progress.task_id)?;
        ensure!(!path.exists(), "下载任务进度文件已存在");
        replace_json(&path, progress)
    }

    pub fn save(&self, progress: &DownloadProgress) -> eyre::Result<()> {
        let _io = self.io.lock();
        let path = progress_path(self.directory(), &progress.task_id)?;
        // Deletion uses the same lock, so a late update cannot recreate its file.
        if !path.exists() {
            return Ok(());
        }
        replace_json(&path, progress)
    }

    pub fn delete(&self, task_id: &str) -> eyre::Result<()> {
        let _io = self.io.lock();
        let path = progress_path(self.directory(), task_id)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub fn contains(&self, task_id: &str) -> eyre::Result<bool> {
        Ok(progress_path(self.directory(), task_id)?.is_file())
    }
}

fn progress_path(directory: &Path, task_id: &str) -> eyre::Result<PathBuf> {
    uuid::Uuid::parse_str(task_id).wrap_err("无效的下载任务 ID")?;
    Ok(directory.join(format!("{task_id}.json")))
}

pub fn read_progress(path: &Path) -> eyre::Result<DownloadProgress> {
    let progress: DownloadProgress = serde_json::from_reader(fs::File::open(path)?)?;
    let expected_path = progress_path(
        path.parent().ok_or_else(|| eyre!("缺少父目录"))?,
        &progress.task_id,
    )?;
    ensure!(path == expected_path, "进度文件名与任务 ID 不符");
    Ok(progress)
}

pub fn read_progresses(directory: &Path) -> eyre::Result<Vec<DownloadProgress>> {
    let mut progresses = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json")
            || !path.is_file()
        {
            continue;
        }
        match read_progress(&path) {
            Ok(progress) => progresses.push(progress),
            Err(err) => {
                tracing::error!(err_title = "恢复进度失败，保留原文件", path = %path.display(), message = ?err)
            }
        }
    }
    Ok(progresses)
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use std::{sync::Arc, thread};

    use super::*;

    fn progress(title: &str) -> DownloadProgress {
        DownloadProgress {
            task_id: "00000000-0000-4000-8000-000000000001".into(),
            episode_title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn writes_and_updates_the_existing_json_format() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProgressStore::new(directory.path().to_owned()).unwrap();
        store.create(&progress("initial")).unwrap();
        store.save(&progress("updated")).unwrap();
        let path = progress_path(directory.path(), &progress("").task_id).unwrap();
        assert_eq!(read_progress(&path).unwrap(), progress("updated"));
        let json: serde_json::Value =
            serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
        assert_eq!(json, serde_json::to_value(progress("updated")).unwrap());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(store.create(&progress("duplicate")).is_err());
    }

    #[test]
    fn concurrent_updates_cannot_recreate_a_deleted_file() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(ProgressStore::new(directory.path().to_owned()).unwrap());
        store.create(&progress("initial")).unwrap();
        let producer_store = store.clone();
        let producer = thread::spawn(move || {
            for index in 0..100 {
                producer_store.save(&progress(&index.to_string())).unwrap();
            }
        });
        store.delete(&progress("").task_id).unwrap();
        producer.join().unwrap();
        store.save(&progress("late update")).unwrap();
        store.delete(&progress("").task_id).unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn failed_deletion_is_reported_and_keeps_the_entry() {
        let directory = tempfile::tempdir().unwrap();
        let store = ProgressStore::new(directory.path().to_owned()).unwrap();
        let path = progress_path(directory.path(), &progress("").task_id).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.delete(&progress("").task_id).is_err());
        assert!(path.is_dir());
    }

    #[test]
    fn serialization_failure_preserves_previous_file_and_cleans_temporary() {
        struct Broken;
        impl Serialize for Broken {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeMap;
                let mut partial = serializer.serialize_map(Some(2))?;
                partial.serialize_entry("partial", &true)?;
                Err(serde::ser::Error::custom("injected serialization failure"))
            }
        }
        let directory = tempfile::tempdir().unwrap();
        let path = progress_path(directory.path(), &progress("").task_id).unwrap();
        replace_json(&path, &progress("valid")).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(replace_json(&path, &Broken).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn reads_legacy_files_with_defaults_for_missing_fields() {
        let directory = tempfile::tempdir().unwrap();
        let task_id = progress("").task_id;
        let path = progress_path(directory.path(), &task_id).unwrap();
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"task_id": task_id, "episode_title": "legacy"}))
                .unwrap(),
        )
        .unwrap();
        let loaded = read_progress(&path).unwrap();
        assert_eq!(loaded.episode_title, "legacy");
        assert!(!loaded.video_task.completed);
    }

    #[test]
    fn restoration_preserves_invalid_and_unrelated_files() {
        let directory = tempfile::tempdir().unwrap();
        let valid = progress("valid");
        replace_json(
            &progress_path(directory.path(), &valid.task_id).unwrap(),
            &valid,
        )
        .unwrap();
        let invalid = directory.path().join("invalid.json");
        fs::write(&invalid, "{truncated").unwrap();
        let unrelated = directory.path().join("notes.txt");
        fs::write(&unrelated, "keep").unwrap();
        assert_eq!(read_progresses(directory.path()).unwrap(), vec![valid]);
        assert_eq!(fs::read_to_string(invalid).unwrap(), "{truncated");
        assert_eq!(fs::read_to_string(unrelated).unwrap(), "keep");
    }

    #[test]
    fn rejects_invalid_ids_and_mismatched_filenames() {
        let directory = tempfile::tempdir().unwrap();
        assert!(progress_path(directory.path(), "../escape").is_err());
        let path = directory
            .path()
            .join(format!("{}.json", uuid::Uuid::new_v4()));
        replace_json(&path, &progress("different ID")).unwrap();
        assert!(read_progress(&path).is_err());
    }
}
