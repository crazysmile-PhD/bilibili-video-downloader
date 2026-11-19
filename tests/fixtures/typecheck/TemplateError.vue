<script setup lang="ts">
import { ref } from 'vue'

const label = ref('hello')
</script>

<template>
  <div>{{ label.missingMethod() }}</div>
</template>
