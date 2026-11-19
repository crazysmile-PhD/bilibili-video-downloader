<script setup lang="ts">
import icon from '../../src-tauri/icons/128x128.png'
import { PhCopySimple, PhMinus, PhPushPin, PhSquare, PhX } from '@phosphor-icons/vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { onMounted, onUnmounted, ref } from 'vue'
import { useStore } from '../store.ts'
import { platform } from '@tauri-apps/plugin-os'
import { ensureHttps } from '../utils.tsx'
import LoginDialog from '../dialogs/LoginDialog.vue'

const store = useStore()

const appWindow = getCurrentWindow()
const windowMaximised = ref<boolean>(false)
const windowFullscreen = ref<boolean>(false)
const windowAlwaysOnTop = ref<boolean>(false)

const currentPlatform = platform()

const loginDialogShowing = ref<boolean>(false)

let disposed = false
let unlistenResize: (() => void) | undefined

async function updateWindowState() {
  windowMaximised.value = await appWindow.isMaximized()
  windowFullscreen.value = await appWindow.isFullscreen()
}

onMounted(async () => {
  try {
    await updateWindowState()
    const unlisten = await appWindow.onResized(() => {
      updateWindowState().catch(console.error)
    })
    if (disposed) unlisten()
    else unlistenResize = unlisten
  } catch (error) {
    console.error('读取窗口状态失败', error)
  }
})

onUnmounted(() => {
  disposed = true
  unlistenResize?.()
})

async function handleAlwaysOnTopButton() {
  const enabled = !windowAlwaysOnTop.value
  try {
    await appWindow.setAlwaysOnTop(enabled)
    windowAlwaysOnTop.value = enabled
  } catch (error) {
    console.error('设置窗口置顶失败', error)
  }
}
</script>

<template>
  <div
    data-tauri-drag-region
    class="flex items-center bg-[#F2F2F2] border-b-solid border-b-1 border-b-[#DADADA] h-9 flex-shrink-0 select-none">
    <div v-if="currentPlatform === 'macos' && !windowFullscreen" class="ml-16" />
    <img data-tauri-drag-region :src="icon" alt="icon" class="ml-2 mr-2 w-6 h-6" draggable="false" />
    <span data-tauri-drag-region class="text-base select-none">哔哩哔哩视频下载器</span>

    <div class="flex whitespace-nowrap ml-auto mr-4" @click="loginDialogShowing = true">
      <div v-if="store.userInfo !== undefined" class="flex items-center whitespace-nowrap cursor-pointer">
        <img
          class="w-8 h-8 rounded-full"
          :src="`${ensureHttps(store.userInfo.face)}@128w_128h`"
          alt=""
          draggable="false" />
        <div class="line-clamp-1">{{ store.userInfo.uname }}</div>
      </div>
      <div v-else class="flex items-center whitespace-nowrap cursor-pointer" @click="loginDialogShowing = true">
        <img
          class="w-8 h-8 rounded-full"
          src="https://i0.hdslb.com/bfs/face/member/noface.jpg@128w_128h"
          alt=""
          draggable="false" />
        <div class="line-clamp-1">未登录</div>
      </div>
    </div>

    <div
      title="窗口置顶"
      class="mr-2 cursor-pointer p-1.5 rounded-lg flex items-center justify-between hover:bg-gray-3/70"
      :class="windowAlwaysOnTop ? 'text-sky-5' : 'text-black'"
      @click="handleAlwaysOnTopButton">
      <PhPushPin size="18" :weight="windowAlwaysOnTop ? 'fill' : 'regular'" />
    </div>

    <div v-if="currentPlatform !== 'macos'" class="flex items-center select-none">
      <div
        class="flex items-center justify-center h-9 w-9 hover:bg-gray/20 cursor-pointer"
        @click="appWindow.minimize()">
        <PhMinus size="16" />
      </div>
      <div
        class="flex items-center justify-center h-9 w-9 hover:bg-gray/20 cursor-pointer"
        @click="appWindow.toggleMaximize()">
        <PhCopySimple v-if="windowMaximised" size="16" />
        <PhSquare v-else size="14" />
      </div>
      <div
        class="flex items-center justify-center h-9 w-9 hover:bg-red hover:text-white cursor-pointer"
        @click="appWindow.close()">
        <PhX size="16" />
      </div>
    </div>
    <LoginDialog v-model:showing="loginDialogShowing" />
  </div>
</template>
