import { readonly, ref } from 'vue'
import type { ProgressData, ProgressPatch, ProgressPatchFactory } from './features/downloads/types.ts'

export type FrameScheduler = (callback: () => void) => number
export type FrameCanceller = (frameId: number) => void

const scheduleOnAnimationFrame: FrameScheduler = (callback) => {
  return requestAnimationFrame(callback)
}

const cancelAnimationFrameUpdate: FrameCanceller = (frameId) => cancelAnimationFrame(frameId)

/**
 * Keeps high-frequency progress writes outside Vue's reactivity graph and
 * propagates only the task records that changed during an animation frame.
 */
export function createProgresses(
  scheduleFrame: FrameScheduler = scheduleOnAnimationFrame,
  cancelFrame: FrameCanceller = cancelAnimationFrameUpdate,
) {
  const source = new Map<string, ProgressData>()
  const progresses = ref<Map<string, ProgressData>>(new Map())
  // Materialized state indexes keep active-task views independent from a
  // potentially large completed-task history.
  const completedProgresses = ref<Map<string, ProgressData>>(new Map())
  const uncompletedProgresses = ref<Map<string, ProgressData>>(new Map())
  const dirtyTaskIds = new Set<string>()
  let frameId: number | undefined
  let disposed = false

  const addToStateIndex = (taskId: string, progress: ProgressData) => {
    if (progress.state === 'Completed') {
      completedProgresses.value.set(taskId, progress)
    } else {
      uncompletedProgresses.value.set(taskId, progress)
    }
  }

  const removeFromStateIndex = (taskId: string, state: ProgressData['state']) => {
    if (state === 'Completed') {
      completedProgresses.value.delete(taskId)
    } else {
      uncompletedProgresses.value.delete(taskId)
    }
  }

  const flush = () => {
    frameId = undefined
    if (disposed) return

    // Clear the current batch first so writes triggered while flushing are
    // scheduled for the next frame instead of being lost.
    const taskIds = Array.from(dirtyTaskIds)
    dirtyTaskIds.clear()

    for (const taskId of taskIds) {
      const next = source.get(taskId)
      if (next === undefined) {
        const current = progresses.value.get(taskId)
        if (current !== undefined) {
          removeFromStateIndex(taskId, current.state)
        }
        progresses.value.delete(taskId)
        continue
      }

      const current = progresses.value.get(taskId)
      if (current === undefined) {
        // Keep the canonical and reactive records separate. Otherwise a write
        // through the canonical object could bypass Vue's proxy.
        const record = { ...next }
        progresses.value.set(taskId, record)
        addToStateIndex(taskId, record)
      } else {
        const previousState = current.state
        Object.assign(current, next)

        if ((previousState === 'Completed') !== (next.state === 'Completed')) {
          removeFromStateIndex(taskId, previousState)
          addToStateIndex(taskId, current)
        }
      }
    }
  }

  const markDirty = (taskId: string) => {
    dirtyTaskIds.add(taskId)

    if (frameId === undefined) {
      frameId = scheduleFrame(flush)
    }
  }

  const setProgress = (progress: ProgressData) => {
    if (disposed) return
    const taskId = progress.task_id
    source.set(taskId, { ...progress })
    markDirty(taskId)
  }

  const patchProgress = (taskId: string, patch: ProgressPatch | ProgressPatchFactory): boolean => {
    const current = source.get(taskId)
    if (disposed || current === undefined) {
      return false
    }

    const resolvedPatch = typeof patch === 'function' ? patch(current) : patch
    source.set(taskId, { ...current, ...resolvedPatch })
    markDirty(taskId)
    return true
  }

  const deleteProgress = (taskId: string): boolean => {
    if (disposed || !source.delete(taskId)) {
      return false
    }

    markDirty(taskId)
    return true
  }

  const replaceProgresses = (records: ProgressData[]) => {
    if (disposed) return
    for (const taskId of source.keys()) dirtyTaskIds.add(taskId)
    source.clear()
    for (const record of records) setProgress(record)
    if (dirtyTaskIds.size > 0 && frameId === undefined) frameId = scheduleFrame(flush)
  }

  const dispose = () => {
    disposed = true
    if (frameId !== undefined) cancelFrame(frameId)
    frameId = undefined
    dirtyTaskIds.clear()
    source.clear()
  }

  return {
    progresses: readonly(progresses),
    completedProgresses: readonly(completedProgresses),
    uncompletedProgresses: readonly(uncompletedProgresses),
    setProgress,
    patchProgress,
    deleteProgress,
    replaceProgresses,
    dispose,
  }
}
