<script setup lang="ts">
import { ref, useTemplateRef } from 'vue'
import { useDownloadEvents } from '../../features/downloads/useDownloadEvents.ts'
import { useStore } from '../../store.ts'
import UncompletedProgresses from './components/UncompletedProgresses.vue'
import CompletedProgresses from './components/CompletedProgresses.vue'
import DownloadDirInput from './components/DownloadDirInput.vue'
import { PhCheckCircle, PhCloudArrowDown } from '@phosphor-icons/vue'
import { NBadge, NPagination, NTabPane, NTabs } from 'naive-ui'

const store = useStore()

const currentTabName = ref<'uncompleted' | 'completed'>('uncompleted')

const uncompletedProgressesRef = useTemplateRef<InstanceType<typeof UncompletedProgresses>>('uncompletedProgressesRef')
const completedProgressesRef = useTemplateRef<InstanceType<typeof CompletedProgresses>>('completedProgressesRef')

useDownloadEvents()
</script>

<template>
  <div class="h-full flex flex-col overflow-auto" v-if="store.config !== undefined">
    <div class="flex items-center m-2 mb-0">
      <DownloadDirInput />
      <span class="ml-2 whitespace-nowrap">下载速度：{{ store.downloadSpeed }}</span>
    </div>
    <n-tabs
      class="h-full overflow-auto"
      type="line"
      v-model:value="currentTabName"
      animated
      size="small"
      placement="bottom">
      <n-tab-pane class="h-full p-0! overflow-auto flex flex-col" name="uncompleted">
        <template #tab>
          <n-badge :value="store.uncompletedProgressesCount" :offset="[5, -3]" class="tab-badge-wrapper">
            <PhCloudArrowDown :weight="currentTabName === 'uncompleted' ? 'fill' : 'regular'" size="20" />
            <span class="ml-1">下载中</span>
          </n-badge>
        </template>
        <UncompletedProgresses ref="uncompletedProgressesRef" />
      </n-tab-pane>

      <n-tab-pane class="h-full p-0! overflow-auto flex flex-col" name="completed">
        <template #tab>
          <PhCheckCircle :weight="currentTabName === 'completed' ? 'fill' : 'regular'" size="20" />
          <span class="ml-1">已完成</span>
        </template>

        <CompletedProgresses ref="completedProgressesRef" />
      </n-tab-pane>

      <template #suffix>
        <n-pagination
          v-if="currentTabName === 'uncompleted' && uncompletedProgressesRef !== null"
          class="ml-auto mr-2"
          :page-count="uncompletedProgressesRef.pageCount"
          v-model:page="uncompletedProgressesRef.currentPage" />

        <n-pagination
          v-else-if="currentTabName === 'completed' && completedProgressesRef !== null"
          class="ml-auto mr-2"
          :page-count="completedProgressesRef.pageCount"
          v-model:page="completedProgressesRef.currentPage" />
      </template>
    </n-tabs>
  </div>
</template>
<style scoped>
:deep(.n-tabs-nav-scroll-wrapper) {
  @apply mt-2 h-9;
}

:deep(.n-tabs-nav__suffix) {
  @apply important-border-0;
}

:deep(.n-tabs-nav-scroll-wrapper) {
  @apply overflow-visible;
}

:deep(.v-x-scroll) {
  @apply overflow-visible;
}
</style>
