use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::Arc,
};

use eyre::{WrapErr, ensure};
use fs4::fs_std::FileExt;
use parking_lot::Mutex;

use crate::extensions::AppHandleExt;

use super::{
    chunk_transfer::transfer_chunks, download_chunk_task::DownloadChunkTask,
    download_progress::DownloadProgress, download_task::DownloadTask, media_chunk::MediaChunk,
};

#[derive(Clone, Copy)]
pub enum MediaKind {
    Video,
    Audio,
}

impl MediaKind {
    fn chunks(self, progress: &mut DownloadProgress) -> &mut Vec<MediaChunk> {
        match self {
            Self::Video => &mut progress.video_task.chunks,
            Self::Audio => &mut progress.audio_task.chunks,
        }
    }
}

/// Open first, then inspect the same handle so resume decisions match the file used.
fn open_partial_file(path: &Path, content_length: u64) -> eyre::Result<(File, bool)> {
    ensure!(content_length > 0, "媒体长度必须大于零");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let reusable = file.metadata()?.len() == content_length;
    if !reusable {
        file.set_len(0)?;
        file.allocate(content_length)?;
        file.set_len(content_length)?;
    }
    Ok((file, reusable))
}

pub async fn download_media(
    download_task: &Arc<DownloadTask>,
    path: &Path,
    url: &str,
    content_length: u64,
    chunks: &[MediaChunk],
    kind: MediaKind,
) -> eyre::Result<()> {
    ensure!(!chunks.is_empty(), "媒体分片列表为空");
    let (file, reusable) = open_partial_file(path, content_length)?;
    let file = Arc::new(Mutex::new(file));
    if !reusable {
        // Completion flags only describe bytes in the original partial file.
        download_task.update_progress(|progress| {
            for chunk in kind.chunks(progress) {
                chunk.completed = false;
            }
        });
    }

    let concurrency = download_task.app.get_config().read().chunk_concurrency;
    let pending = chunks
        .iter()
        .enumerate()
        .filter(|(_, chunk)| !reusable || !chunk.completed)
        .map(|(index, _)| index);

    transfer_chunks(
        pending,
        concurrency,
        |chunk_index| {
            let chunk = &chunks[chunk_index];
            let (start, end) = (chunk.start, chunk.end);
            let task = DownloadChunkTask {
                download_task: download_task.clone(),
                start,
                end,
                url: url.to_owned(),
                file: file.clone(),
                chunk_index,
            };
            async move {
                task.process()
                    .await
                    .wrap_err_with(|| format!("分片 {} 下载失败 ({start}-{end})", chunk_index + 1))
            }
        },
        |index| {
            download_task.update_progress(|progress| kind.chunks(progress)[index].completed = true);
        },
    )
    .await?;

    // Release the file handle before the caller validates and renames it on Windows.
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPath(std::path::PathBuf);

    impl TestPath {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("bili-partial-{}", uuid::Uuid::new_v4())))
        }
    }

    impl Drop for TestPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn missing_or_resized_files_invalidate_resume_state() {
        let path = TestPath::new();
        let (file, reusable) = open_partial_file(&path.0, 8).unwrap();
        assert!(!reusable);
        assert_eq!(file.metadata().unwrap().len(), 8);
        drop(file);

        let (file, reusable) = open_partial_file(&path.0, 4).unwrap();
        assert!(!reusable);
        assert_eq!(file.metadata().unwrap().len(), 4);
    }

    #[test]
    fn matching_file_preserves_downloaded_bytes() {
        let path = TestPath::new();
        std::fs::write(&path.0, [1, 2, 3, 4]).unwrap();
        let (file, reusable) = open_partial_file(&path.0, 4).unwrap();
        assert!(reusable);
        drop(file);
        assert_eq!(std::fs::read(&path.0).unwrap(), [1, 2, 3, 4]);
    }
}
