import { defineStore } from 'pinia'
import { ref, computed, readonly, onScopeDispose } from 'vue'
import type { Config, DownloadEvent, RestoredDownloadTask, UserInfo } from './bindings.ts'
import type { CurrentNavName } from './types/navigation.ts'
import type { ProgressData } from './features/downloads/types.ts'
import { applyDownloadEvent } from './features/downloads/events.ts'
import { createProgressData } from './features/downloads/progress.ts'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const userInfo = ref<UserInfo>()
  const currentNavName = ref<CurrentNavName>('search')
  const downloadSpeed = ref<string>('')

  const { progresses, updateProgresses } = useProgresses()
  const uncompletedProgressesCount = computed<number>(() => {
    return Array.from(progresses.value.values()).filter(({ state }) => state !== 'Completed').length
  })

  function handleDownloadEvent(event: DownloadEvent) {
    if (event.event === 'Speed') {
      downloadSpeed.value = event.data.speed
      return
    }
    updateProgresses((progresses) => applyDownloadEvent(progresses, event))
  }

  function restoreProgresses(tasks: RestoredDownloadTask[]) {
    updateProgresses((progresses) => {
      progresses.clear()
      for (const { state, progress } of tasks) {
        progresses.set(progress.task_id, createProgressData(progress, state))
      }
    })
  }

  return {
    config,
    currentNavName,
    userInfo,
    progresses,
    updateProgresses,
    uncompletedProgressesCount,
    downloadSpeed,
    handleDownloadEvent,
    restoreProgresses,
  }
})

function useProgresses() {
  // 对外暴露的响应式状态
  const progresses = ref<Map<string, ProgressData>>(new Map())

  // 等待在同一渲染帧内执行的更新函数
  const pendingUpdateFns: Array<(progresses: Map<string, ProgressData>) => void> = []

  // 用于确保在同一渲染帧内只安排一次UI更新
  let frameId: number | undefined

  // 在同一渲染帧内集中执行等待中的更新函数
  const updateProgressesOnFrame = () => {
    frameId = undefined

    const updateFns = pendingUpdateFns.splice(0)
    for (const updateFn of updateFns) {
      updateFn(progresses.value)
    }
  }

  const updateProgresses = (updateFn: (progresses: Map<string, ProgressData>) => void) => {
    // 将传入的更新函数添加到等待队列
    pendingUpdateFns.push(updateFn)

    if (frameId === undefined) {
      // 如果没有安排过UI更新，则安排一次
      // 使用 `requestAnimationFrame` 调度 UI 更新
      frameId = requestAnimationFrame(updateProgressesOnFrame)
    }
  }

  onScopeDispose(() => {
    if (frameId !== undefined) cancelAnimationFrame(frameId)
    pendingUpdateFns.length = 0
  })

  return { progresses: readonly(progresses), updateProgresses }
}
