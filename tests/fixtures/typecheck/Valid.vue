<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{ count: number }>()
const label = ref('hello')
</script>

<template>
  <div>{{ props.count }} {{ label.toUpperCase() }}</div>
</template>
