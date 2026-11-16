import type { DownloadEvent } from '../../bindings.ts'
import { createProgressData, getProgressPresentation, updateProgressState } from './progress.ts'
import type { ProgressMutations } from './types.ts'

/** Apply backend events in arrival order; no Vue or Tauri runtime is needed here. */
export function applyDownloadEvent(progresses: ProgressMutations, event: DownloadEvent) {
  switch (event.event) {
    case 'Speed':
      return
    case 'TaskCreate':
      progresses.setProgress(createProgressData(event.data.progress, event.data.state))
      return
    case 'TaskDelete':
      progresses.deleteProgress(event.data.task_id)
      return
    case 'TaskStateUpdate': {
      progresses.patchProgress(event.data.task_id, (current) => {
        const progress = { ...current }
        updateProgressState(progress, event.data.state)
        return progress
      })
      return
    }
    case 'TaskSleeping': {
      progresses.patchProgress(event.data.task_id, { taskIndicator: `将在${event.data.remaining_sec}秒后继续下载` })
      return
    }
    case 'ProgressPreparing': {
      progresses.patchProgress(event.data.task_id, { taskIndicator: '正在准备下载' })
      return
    }
    case 'ProgressUpdate': {
      progresses.patchProgress(event.data.progress.task_id, (current) => {
        const progress = { ...current, ...event.data.progress }
        return { ...progress, ...getProgressPresentation(progress, progress.state) }
      })
    }
  }
}
