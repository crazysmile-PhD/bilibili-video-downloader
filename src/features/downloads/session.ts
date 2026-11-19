import type { DownloadEvent, RestoredDownloadTask } from '../../bindings.ts'

type Unlisten = () => void

/** Subscribe before restoring, then replay live events over the restored snapshot. */
export async function startDownloadSession(
  subscribe: (handle: (event: DownloadEvent) => void) => Promise<Unlisten>,
  restore: () => Promise<RestoredDownloadTask[]>,
  initialize: (tasks: RestoredDownloadTask[]) => void,
  handle: (event: DownloadEvent) => void,
  signal: AbortSignal,
): Promise<Unlisten> {
  const pending: DownloadEvent[] = []
  let restoring = true
  let unlisten: Unlisten | undefined
  let stopped = signal.aborted
  const stop = () => {
    stopped = true
    pending.length = 0
    unlisten?.()
    unlisten = undefined
    signal.removeEventListener('abort', stop)
  }
  if (stopped) return stop
  signal.addEventListener('abort', stop, { once: true })

  try {
    unlisten = await subscribe((event) => {
      if (stopped) return
      if (restoring) pending.push(event)
      else handle(event)
    })
    if (stopped) {
      stop()
      return stop
    }
    try {
      const tasks = await restore()
      if (!stopped) initialize(tasks)
    } finally {
      restoring = false
      if (!stopped) {
        for (const event of pending) handle(event)
      }
      pending.length = 0
    }
    return stop
  } catch (error) {
    stop()
    throw error
  }
}
