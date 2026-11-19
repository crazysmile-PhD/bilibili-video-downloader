import type { DownloadProgress, DownloadTaskState } from '../../bindings.ts'

export type ProgressData = DownloadProgress & {
  state: DownloadTaskState
  percentage: number
  stateIndicator: string
  taskIndicator: string
}

export type ProgressPatch = Partial<ProgressData>
export type ProgressPatchFactory = (progress: Readonly<ProgressData>) => ProgressPatch

export interface ProgressMutations {
  setProgress(progress: ProgressData): void
  patchProgress(taskId: string, patch: ProgressPatch | ProgressPatchFactory): boolean
  deleteProgress(taskId: string): boolean
}
