import { defineStore } from 'pinia'
import { ref, computed, onScopeDispose } from 'vue'
import type { Config, DownloadEvent, RestoredDownloadTask, UserInfo } from './bindings.ts'
import type { CurrentNavName } from './types/navigation.ts'
import { createProgresses } from './progresses.ts'
import { applyDownloadEvent } from './features/downloads/events.ts'
import { createProgressData } from './features/downloads/progress.ts'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const userInfo = ref<UserInfo>()
  const currentNavName = ref<CurrentNavName>('search')
  const downloadSpeed = ref<string>('')

  const progressStore = createProgresses()
  const { progresses, completedProgresses, uncompletedProgresses } = progressStore
  const uncompletedProgressesCount = computed(() => uncompletedProgresses.value.size)
  onScopeDispose(progressStore.dispose)

  function handleDownloadEvent(event: DownloadEvent) {
    if (event.event === 'Speed') {
      downloadSpeed.value = event.data.speed
      return
    }
    applyDownloadEvent(progressStore, event)
  }

  function restoreProgresses(tasks: RestoredDownloadTask[]) {
    progressStore.replaceProgresses(tasks.map(({ state, progress }) => createProgressData(progress, state)))
  }

  return {
    config,
    currentNavName,
    userInfo,
    progresses,
    completedProgresses,
    uncompletedProgresses,
    uncompletedProgressesCount,
    downloadSpeed,
    handleDownloadEvent,
    restoreProgresses,
  }
})
