use std::future::Future;

use eyre::{WrapErr, eyre};
use tokio::task::JoinSet;
use tracing::Instrument;

#[derive(Debug, PartialEq, Eq)]
pub enum DownloadChunkResult {
    Downloaded(usize),
    Interrupted,
}

/// Keep only a bounded window of jobs alive, even for very large media files.
/// Dropping this future also drops the JoinSet and aborts outstanding jobs.
pub async fn transfer_chunks<F, Fut>(
    mut pending: impl Iterator<Item = usize>,
    concurrency: usize,
    mut download: F,
    mut on_completed: impl FnMut(usize),
) -> eyre::Result<()>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = eyre::Result<DownloadChunkResult>> + Send + 'static,
{
    let mut jobs = JoinSet::new();
    let concurrency = concurrency.max(1);

    loop {
        while jobs.len() < concurrency {
            let Some(index) = pending.next() else {
                break;
            };
            jobs.spawn(download(index).in_current_span());
        }

        let Some(result) = jobs.join_next().await else {
            return Ok(());
        };

        match result.wrap_err("分片下载工作异常退出")?? {
            DownloadChunkResult::Downloaded(index) => on_completed(index),
            DownloadChunkResult::Interrupted => return Err(eyre!("分片下载已中断")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    #[tokio::test]
    async fn bounds_live_jobs_and_preserves_completed_indices() {
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let mut completed = Vec::new();
        transfer_chunks(
            (0..100).filter(|i| i % 2 == 0),
            3,
            |index| {
                let active = active.clone();
                let peak = peak.clone();
                async move {
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(count, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(DownloadChunkResult::Downloaded(index))
                }
            },
            |index| completed.push(index),
        )
        .await
        .unwrap();
        completed.sort_unstable();
        assert_eq!(completed, (0..100).step_by(2).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= 3);
    }

    #[tokio::test]
    async fn interruption_never_marks_a_chunk_complete_or_schedules_more_jobs() {
        let mut scheduled = 0;
        let mut completed = Vec::new();
        let result = transfer_chunks(
            0..10,
            1,
            |_| {
                scheduled += 1;
                async { Ok(DownloadChunkResult::Interrupted) }
            },
            |index| completed.push(index),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(scheduled, 1);
        assert!(completed.is_empty());
    }

    #[tokio::test]
    async fn propagates_download_errors_and_panics() {
        let error = transfer_chunks(
            0..1,
            1,
            |_| async { Err(eyre!("network failure")) },
            |_| panic!("failed chunks must not complete"),
        )
        .await;
        assert!(error.unwrap_err().to_string().contains("network failure"));

        let panic = transfer_chunks(
            0..1,
            1,
            |_| async { panic!("worker panic") },
            |_| panic!("panicked chunks must not complete"),
        )
        .await;
        assert!(panic.is_err());
    }

    #[tokio::test]
    async fn zero_concurrency_still_makes_progress() {
        let mut completed = Vec::new();
        transfer_chunks(
            0..2,
            0,
            |index| async move { Ok(DownloadChunkResult::Downloaded(index)) },
            |index| completed.push(index),
        )
        .await
        .unwrap();
        assert_eq!(completed, vec![0, 1]);
    }

    #[tokio::test]
    async fn dropping_transfer_aborts_outstanding_workers() {
        struct WorkerGuard(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for WorkerGuard {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }

        let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
        let (dropped_sender, dropped_receiver) = tokio::sync::oneshot::channel();
        let mut signals = Some((started_sender, dropped_sender));
        let transfer = tokio::spawn(async move {
            transfer_chunks(
                0..1,
                1,
                |_| {
                    let (started, dropped) = signals.take().unwrap();
                    async move {
                        let _guard = WorkerGuard(Some(dropped));
                        let _ = started.send(());
                        std::future::pending::<eyre::Result<DownloadChunkResult>>().await
                    }
                },
                |_| panic!("aborted jobs must not complete"),
            )
            .await
        });
        started_receiver.await.unwrap();
        transfer.abort();
        assert!(transfer.await.unwrap_err().is_cancelled());
        tokio::time::timeout(std::time::Duration::from_secs(1), dropped_receiver)
            .await
            .expect("worker must be aborted with its scheduler")
            .unwrap();
    }
}
