import assert from 'node:assert/strict'
import { test } from 'node:test'
import { computed, toRaw } from 'vue'
import { createProgressData } from '../src/features/downloads/progress.ts'
import { applyDownloadEvent } from '../src/features/downloads/events.ts'
import { makeProgress } from './download-fixtures.mjs'
import { createProgressHarness } from './progress-fixtures.mjs'

function progress(taskId, percentage = 0, state = 'Downloading') {
  return { ...createProgressData(makeProgress({ task_id: taskId }), state), percentage }
}

test('coalesces repeated writes to the same task into one reactive update', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('task-1'))
  frames.runFrame()

  let writes = 0
  let value = 0
  Object.defineProperty(toRaw(progresses.get('task-1')), 'percentage', {
    configurable: true,
    get: () => value,
    set: (next) => {
      writes++
      value = next
    },
  })
  for (let percentage = 1; percentage <= 100; percentage++) {
    assert.equal(store.patchProgress('task-1', { percentage }), true)
  }
  assert.equal(frames.pendingFrames, 1)
  assert.equal(progresses.get('task-1').percentage, 0)
  frames.runFrame()
  assert.equal(progresses.get('task-1').percentage, 100)
  assert.equal(writes, 1)
  assert.equal(frames.pendingFrames, 0)
})

test('updating one active task leaves 4999 completed records and their list untouched', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('task-0'))
  for (let index = 1; index < 5000; index++) {
    store.setProgress(progress(`task-${index}`, 100, 'Completed'))
  }
  assert.equal(frames.pendingFrames, 1)
  frames.runFrame()
  assert.equal(store.uncompletedProgresses.value.size, 1)
  assert.equal(store.completedProgresses.value.size, 4999)

  let cleanWrites = 0
  for (const record of store.completedProgresses.value.values()) {
    let indicator = record.stateIndicator
    Object.defineProperty(toRaw(record), 'stateIndicator', {
      configurable: true,
      get: () => indicator,
      set: (value) => {
        cleanWrites++
        indicator = value
      },
    })
  }
  let listRuns = 0
  const completedList = computed(() => {
    listRuns++
    return Array.from(store.completedProgresses.value.values()).sort((a, b) => b.completed_ts - a.completed_ts)
  })
  const previousList = completedList.value
  store.patchProgress('task-0', { percentage: 50 })
  frames.runFrame()
  assert.equal(progresses.get('task-0').percentage, 50)
  assert.equal(cleanWrites, 0)
  assert.equal(completedList.value, previousList)
  assert.equal(listRuns, 1)
})

test('preserves delete, recreate and update order before a frame is rendered', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('task-1', 10))
  assert.equal(store.patchProgress('task-1', { percentage: 20 }), true)
  assert.equal(store.deleteProgress('task-1'), true)
  assert.equal(store.patchProgress('task-1', { percentage: 30 }), false)
  store.setProgress(progress('task-1', 40))
  assert.equal(store.patchProgress('task-1', { percentage: 50 }), true)
  assert.equal(frames.pendingFrames, 1)
  frames.runFrame()
  assert.equal(progresses.get('task-1').percentage, 50)
})

test('updates both state indexes on completion, restart, and deletion', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('task-1'))
  frames.runFrame()
  const record = progresses.get('task-1')
  assert.equal(store.uncompletedProgresses.value.get('task-1'), record)

  store.patchProgress('task-1', { state: 'Completed' })
  frames.runFrame()
  assert.equal(store.uncompletedProgresses.value.size, 0)
  assert.equal(store.completedProgresses.value.get('task-1'), record)

  store.patchProgress('task-1', { state: 'Pending' })
  frames.runFrame()
  assert.equal(store.completedProgresses.value.size, 0)
  assert.equal(store.uncompletedProgresses.value.get('task-1'), record)
  store.deleteProgress('task-1')
  frames.runFrame()
  assert.equal(progresses.size, 0)
  assert.equal(store.uncompletedProgresses.value.size, 0)
  assert.equal(store.completedProgresses.value.size, 0)
})

test('snapshot replacement drops old and pending tasks and replays events in the same frame', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('old', 100, 'Completed'))
  frames.runFrame()
  store.setProgress(progress('pending'))
  store.replaceProgresses([progress('restored', 20, 'Paused')])
  applyDownloadEvent(store, { event: 'TaskStateUpdate', data: { task_id: 'restored', state: 'Downloading' } })
  applyDownloadEvent(store, { event: 'ProgressPreparing', data: { task_id: 'restored' } })
  frames.runFrame()
  assert.deepEqual([...progresses.keys()], ['restored'])
  assert.equal(progresses.get('restored').state, 'Downloading')
  assert.equal(progresses.get('restored').taskIndicator, '正在准备下载')
  assert.equal(store.completedProgresses.value.size, 0)
  assert.equal(store.uncompletedProgresses.value.size, 1)

  store.replaceProgresses([])
  frames.runFrame()
  assert.equal(progresses.size, 0)
  assert.equal(store.uncompletedProgresses.value.size, 0)
})

test('disposing cancels the pending frame and rejects late updates', () => {
  const frames = createProgressHarness()
  const { store, progresses } = frames
  store.setProgress(progress('task-1'))
  assert.equal(frames.pendingFrames, 1)
  store.dispose()
  assert.equal(frames.pendingFrames, 0)
  store.setProgress(progress('task-2'))
  store.replaceProgresses([progress('task-3')])
  assert.equal(store.patchProgress('task-1', { percentage: 50 }), false)
  assert.equal(frames.pendingFrames, 0)
  assert.equal(progresses.size, 0)
})
