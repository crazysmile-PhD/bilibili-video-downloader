export function makeProgress(overrides = {}) {
  const mediaTask = () => ({
    selected: false,
    completed: false,
    skipped: false,
    chunks: [],
    content_length: 0,
    url: '',
  })
  return {
    task_id: 'task-1',
    episode_title: 'Test episode',
    video_task: { ...mediaTask(), video_quality: 'Unknown', codec_type: 'Unknown' },
    audio_task: { ...mediaTask(), audio_quality: 'Unknown' },
    video_process_task: {
      merge_selected: false,
      embed_chapter_selected: false,
      embed_skip_selected: false,
      completed: false,
      skipped: false,
    },
    danmaku_task: { xml_selected: false, ass_selected: false, json_selected: false, completed: false },
    subtitle_task: { selected: false, completed: false },
    cover_task: { selected: false, completed: false },
    nfo_task: { selected: false, completed: false },
    json_task: { selected: false, completed: false },
    ...overrides,
  }
}

export function deferred() {
  let resolve
  let reject
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
