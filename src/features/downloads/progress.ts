import type { DownloadProgress, DownloadTaskState, MediaChunk } from '../../bindings.ts'
import type { ProgressData } from './types.ts'

const stateIndicators: Record<DownloadTaskState, string> = {
  Pending: '排队中',
  Downloading: '下载中',
  Paused: '已暂停',
  Completed: '下载完成',
  Failed: '下载失败',
}

function chunkPresentation(chunks: MediaChunk[], label: string) {
  let totalBytes = 0
  let completedBytes = 0
  let completedChunks = 0
  for (const chunk of chunks) {
    const bytes = Math.max(0, chunk.end - chunk.start + 1)
    totalBytes += bytes
    if (chunk.completed) {
      completedBytes += bytes
      completedChunks += 1
    }
  }
  return {
    percentage: totalBytes > 0 ? (completedBytes / totalBytes) * 100 : 0,
    taskIndicator: `${label}分片 ${completedChunks}/${chunks.length}`,
  }
}

export function getProgressPresentation(progress: DownloadProgress, state: DownloadTaskState) {
  const presentation = (percentage: number, taskIndicator: string) => ({ percentage, taskIndicator })
  if (state === 'Completed') return presentation(100, '')

  if (progress.video_task.selected && !progress.video_task.completed) {
    return chunkPresentation(progress.video_task.chunks, '视频')
  }
  if (progress.audio_task.selected && !progress.audio_task.completed) {
    return chunkPresentation(progress.audio_task.chunks, '音频')
  }

  const process = progress.video_process_task
  const embedSelected = process.embed_chapter_selected || process.embed_skip_selected
  if (!process.completed && (process.merge_selected || embedSelected)) {
    if (process.merge_selected && embedSelected) return presentation(100, '自动合并+嵌入章节元数据')
    return presentation(100, process.merge_selected ? '自动合并' : '嵌入章节元数据')
  }

  const danmaku = progress.danmaku_task
  if (!danmaku.completed && (danmaku.xml_selected || danmaku.ass_selected || danmaku.json_selected)) {
    return presentation(100, '弹幕')
  }
  const remainingTasks = [
    [progress.subtitle_task, '字幕'],
    [progress.cover_task, '封面'],
    [progress.nfo_task, 'nfo刮削'],
    [progress.json_task, 'json刮削'],
  ] as const
  for (const [task, label] of remainingTasks) {
    if (task.selected && !task.completed) return presentation(100, label)
  }
  return presentation(100, '')
}

export function createProgressData(progress: DownloadProgress, state: DownloadTaskState): ProgressData {
  return {
    ...progress,
    state,
    stateIndicator: stateIndicators[state],
    ...getProgressPresentation(progress, state),
  }
}

export function updateProgressState(progress: ProgressData, state: DownloadTaskState) {
  progress.state = state
  progress.stateIndicator = stateIndicators[state]
  Object.assign(progress, getProgressPresentation(progress, state))
}
