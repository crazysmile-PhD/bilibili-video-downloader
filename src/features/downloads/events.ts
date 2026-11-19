import type { DownloadEvent } from '../../bindings.ts'
import { createProgressData, getProgressPresentation, updateProgressState } from './progress.ts'
import type { ProgressData } from './types.ts'

/** Apply backend events in arrival order; no Vue or Tauri runtime is needed here. */
export function applyDownloadEvent(progresses: Map<string, ProgressData>, event: DownloadEvent) {
  switch (event.event) {
    case 'Speed':
      return
    case 'TaskCreate':
      progresses.set(event.data.progress.task_id, createProgressData(event.data.progress, event.data.state))
      return
    case 'TaskDelete':
      progresses.delete(event.data.task_id)
      return
    case 'TaskStateUpdate': {
      const progress = progresses.get(event.data.task_id)
      if (progress) updateProgressState(progress, event.data.state)
      return
    }
    case 'TaskSleeping': {
      const progress = progresses.get(event.data.task_id)
      if (progress) progress.taskIndicator = `将在${event.data.remaining_sec}秒后继续下载`
      return
    }
    case 'ProgressPreparing': {
      const progress = progresses.get(event.data.task_id)
      if (progress) progress.taskIndicator = '正在准备下载'
      return
    }
    case 'ProgressUpdate': {
      const progress = progresses.get(event.data.progress.task_id)
      if (progress) {
        Object.assign(progress, event.data.progress)
        Object.assign(progress, getProgressPresentation(progress, progress.state))
      }
    }
  }
}
