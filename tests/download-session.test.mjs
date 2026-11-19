import assert from 'node:assert/strict'
import { test } from 'node:test'
import { startDownloadSession } from '../src/features/downloads/session.ts'
import { applyDownloadEvent } from '../src/features/downloads/events.ts'
import { createProgressData } from '../src/features/downloads/progress.ts'
import { deferred, makeProgress } from './download-fixtures.mjs'

test('events received while restoring are applied after the snapshot in arrival order', async () => {
  const snapshot = deferred()
  const controller = new AbortController()
  const progresses = new Map()
  let receive
  let unlistened = 0
  const session = startDownloadSession(
    async (handle) => {
      receive = handle
      return () => unlistened++
    },
    () => snapshot.promise,
    (tasks) => {
      for (const { progress, state } of tasks) {
        progresses.set(progress.task_id, createProgressData(progress, state))
      }
    },
    (event) => applyDownloadEvent(progresses, event),
    controller.signal,
  )
  receive({ event: 'TaskStateUpdate', data: { task_id: 'task-1', state: 'Downloading' } })
  receive({ event: 'TaskDelete', data: { task_id: 'task-1' } })
  snapshot.resolve([{ progress: makeProgress(), state: 'Paused' }])
  const stop = await session
  assert.equal(progresses.size, 0)

  receive({ event: 'TaskCreate', data: { progress: makeProgress(), state: 'Pending' } })
  assert.equal(progresses.size, 1)
  stop()
  stop()
  controller.abort()
  assert.equal(unlistened, 1)
  receive({ event: 'TaskDelete', data: { task_id: 'task-1' } })
  assert.equal(progresses.size, 1)
})

test('unmount during asynchronous subscription immediately removes the late listener', async () => {
  const subscription = deferred()
  const controller = new AbortController()
  let unlistened = 0
  let restored = false
  const session = startDownloadSession(
    () => subscription.promise,
    async () => {
      restored = true
      return []
    },
    () => assert.fail('must not initialize an unmounted session'),
    () => assert.fail('must not update an unmounted session'),
    controller.signal,
  )
  controller.abort()
  subscription.resolve(() => unlistened++)
  await session
  assert.equal(unlistened, 1)
  assert.equal(restored, false)
})

test('unmount during restoration discards the snapshot and buffered events', async () => {
  const snapshot = deferred()
  const controller = new AbortController()
  let receive
  let unlistened = 0
  const session = startDownloadSession(
    async (handle) => {
      receive = handle
      return () => unlistened++
    },
    () => snapshot.promise,
    () => assert.fail('must not initialize an unmounted session'),
    () => assert.fail('must not replay events after unmount'),
    controller.signal,
  )
  await Promise.resolve()
  receive({ event: 'TaskDelete', data: { task_id: 'task-1' } })
  controller.abort()
  snapshot.resolve([{ progress: makeProgress(), state: 'Paused' }])
  await session
  assert.equal(unlistened, 1)
})

test('restore failures release the listener and surface the error', async () => {
  const controller = new AbortController()
  let unlistened = 0
  await assert.rejects(
    startDownloadSession(
      async () => () => unlistened++,
      async () => {
        throw new Error('restore failed')
      },
      () => assert.fail('failed restore must not initialize'),
      () => {},
      controller.signal,
    ),
    /restore failed/,
  )
  assert.equal(unlistened, 1)
})

test('already aborted sessions perform no subscription or restoration', async () => {
  const controller = new AbortController()
  controller.abort()
  await startDownloadSession(
    async () => assert.fail('must not subscribe'),
    async () => assert.fail('must not restore'),
    () => assert.fail('must not initialize'),
    () => assert.fail('must not handle events'),
    controller.signal,
  )
})
