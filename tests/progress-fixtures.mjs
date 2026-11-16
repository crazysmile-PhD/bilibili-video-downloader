import assert from 'node:assert/strict'
import { createProgresses } from '../src/progresses.ts'
import { applyDownloadEvent } from '../src/features/downloads/events.ts'

export function createProgressHarness() {
  const callbacks = new Map()
  let nextId = 0
  const store = createProgresses(
    (callback) => {
      const id = nextId++
      callbacks.set(id, callback)
      return id
    },
    (id) => callbacks.delete(id),
  )
  const runFrame = () => {
    const first = callbacks.entries().next().value
    assert.ok(first, 'expected a pending animation frame')
    const [id, callback] = first
    callbacks.delete(id)
    callback()
  }

  return {
    store,
    progresses: store.progresses.value,
    get pendingFrames() {
      return callbacks.size
    },
    runFrame,
    apply(event) {
      applyDownloadEvent(store, event)
      if (callbacks.size > 0) runFrame()
    },
  }
}
