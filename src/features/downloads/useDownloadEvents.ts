import { onMounted, onScopeDispose } from 'vue'
import { commands, events } from '../../bindings.ts'
import { useStore } from '../../store.ts'
import { startDownloadSession } from './session.ts'

export function useDownloadEvents() {
  const store = useStore()
  const controller = new AbortController()
  onScopeDispose(() => controller.abort())

  onMounted(async () => {
    try {
      await startDownloadSession(
        (handle) => events.downloadEvent.listen(({ payload }) => handle(payload)),
        async () => {
          const result = await commands.restoreDownloadTasks()
          if (result.status === 'error') {
            console.error('恢复下载任务失败', result.error)
            return []
          }
          return result.data
        },
        store.restoreProgresses,
        store.handleDownloadEvent,
        controller.signal,
      )
    } catch (error) {
      console.error('初始化下载任务失败', error)
    }
  })
}
