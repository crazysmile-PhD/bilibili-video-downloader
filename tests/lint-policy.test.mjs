import assert from 'node:assert/strict'
import { test } from 'node:test'
import { ESLint } from 'eslint'

const eslint = new ESLint()
const filePath = 'src/features/downloads/progress.ts'

test('CI lint policy rejects UI dependencies in domain modules', async () => {
  const [result] = await eslint.lintText("import { ref } from 'vue'\nexport const value = ref(0)\n", { filePath })
  assert.ok(result.messages.some((message) => message.ruleId === 'no-restricted-imports'))
})

test('CI lint policy rejects unhandled promises and accepts explicit failure handling', async () => {
  const [invalid] = await eslint.lintText('export function run() { Promise.resolve(1) }', { filePath })
  assert.ok(invalid.messages.some((message) => message.ruleId === '@typescript-eslint/no-floating-promises'))
  const [valid] = await eslint.lintText('export function run() { Promise.resolve(1).catch(console.error) }', {
    filePath,
  })
  assert.equal(valid.errorCount, 0, JSON.stringify(valid.messages))
})

test('CI lint policy checks unhandled promises inside Vue scripts', async () => {
  const [result] = await eslint.lintText(
    '<script setup lang="ts">\nPromise.resolve(1)\n</script>\n<template><div /></template>',
    { filePath: 'src/AppContent.vue' },
  )
  assert.ok(
    result.messages.some((message) => message.ruleId === '@typescript-eslint/no-floating-promises'),
    JSON.stringify(result.messages),
  )
})

test('CI lint policy rejects promises returned to synchronous callbacks', async () => {
  const [result] = await eslint.lintText('export function run() { [1].forEach(async () => {}) }', { filePath })
  assert.ok(
    result.messages.some((message) => message.ruleId === '@typescript-eslint/no-misused-promises'),
    JSON.stringify(result.messages),
  )
})
