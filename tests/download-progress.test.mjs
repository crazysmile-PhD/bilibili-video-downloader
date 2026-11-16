import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createProgressData, getProgressPresentation } from '../src/features/downloads/progress.ts'
import { makeProgress } from './download-fixtures.mjs'
import { createProgressHarness } from './progress-fixtures.mjs'

test('restored completed tasks immediately display 100 percent and their state', () => {
  const progress = createProgressData(makeProgress(), 'Completed')
  assert.equal(progress.percentage, 100)
  assert.equal(progress.stateIndicator, '下载完成')
  assert.equal(progress.taskIndicator, '')
})

test('partial progress accounts for the shorter last chunk', () => {
  const progress = makeProgress()
  progress.video_task.selected = true
  progress.video_task.content_length = 10
  progress.video_task.chunks = [
    { start: 0, end: 7, completed: true },
    { start: 8, end: 9, completed: false },
  ]
  assert.deepEqual(getProgressPresentation(progress, 'Paused'), {
    percentage: 80,
    taskIndicator: '视频分片 1/2',
  })
})

test('selected media with no chunks has finite zero progress', () => {
  const progress = makeProgress()
  progress.audio_task.selected = true
  const result = getProgressPresentation(progress, 'Downloading')
  assert.equal(result.percentage, 0)
  assert.equal(result.taskIndicator, '音频分片 0/0')
})

test('unselected video processing does not hide remaining subtitle work', () => {
  const progress = makeProgress()
  progress.subtitle_task.selected = true
  assert.equal(getProgressPresentation(progress, 'Downloading').taskIndicator, '字幕')
  progress.video_process_task.embed_skip_selected = true
  assert.equal(getProgressPresentation(progress, 'Downloading').taskIndicator, '嵌入章节元数据')
  progress.video_process_task.merge_selected = true
  assert.equal(getProgressPresentation(progress, 'Downloading').taskIndicator, '自动合并+嵌入章节元数据')
})

test('ordered events preserve task state and object identity across progress updates', () => {
  const { progresses, apply } = createProgressHarness()
  apply({ event: 'TaskCreate', data: { progress: makeProgress(), state: 'Pending' } })
  const original = progresses.get('task-1')
  apply({ event: 'TaskStateUpdate', data: { task_id: 'task-1', state: 'Downloading' } })
  const progress = makeProgress({ episode_title: 'Updated episode' })
  progress.cover_task.selected = true
  apply({ event: 'ProgressUpdate', data: { progress } })
  assert.equal(progresses.get('task-1'), original)
  assert.equal(original.state, 'Downloading')
  assert.equal(original.episode_title, 'Updated episode')
  assert.equal(original.taskIndicator, '封面')

  apply({ event: 'TaskSleeping', data: { task_id: 'task-1', remaining_sec: 2 } })
  assert.equal(original.taskIndicator, '将在2秒后继续下载')
  apply({ event: 'TaskStateUpdate', data: { task_id: 'task-1', state: 'Completed' } })
  assert.equal(original.percentage, 100)
  assert.equal(original.taskIndicator, '')
})

test('late updates cannot recreate a deleted task', () => {
  const { progresses, apply } = createProgressHarness()
  const progress = makeProgress()
  apply({ event: 'TaskCreate', data: { progress, state: 'Paused' } })
  apply({ event: 'TaskDelete', data: { task_id: progress.task_id } })
  apply({ event: 'ProgressUpdate', data: { progress } })
  apply({ event: 'ProgressPreparing', data: { task_id: progress.task_id } })
  assert.equal(progresses.size, 0)
})
