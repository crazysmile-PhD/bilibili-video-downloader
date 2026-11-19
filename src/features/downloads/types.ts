import type { DownloadProgress, DownloadTaskState } from '../../bindings.ts'

export type ProgressData = DownloadProgress & {
  state: DownloadTaskState
  percentage: number
  stateIndicator: string
  taskIndicator: string
}
