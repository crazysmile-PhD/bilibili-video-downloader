import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { test } from 'node:test'

const require = createRequire(import.meta.url)
const nativePackage = require('typescript/package.json')
const vueRequire = createRequire(require.resolve('vue-tsc'))
const bridgePackage = vueRequire('typescript/package.json')
const vueCli = join(dirname(require.resolve('vue-tsc/package.json')), 'bin/vue-tsc.js')

function checkVue(project) {
  const result = spawnSync(process.execPath, [vueCli, '--noEmit', '--project', project], {
    encoding: 'utf8',
    timeout: 60_000,
    env: process.env,
  })
  assert.ifError(result.error)
  assert.equal(result.signal, null, result.stderr)
  assert.match(result.stderr, /TNB ACTIVE/, 'Vue checks must load the native checker adapter')
  return { status: result.status, output: result.stdout + result.stderr }
}

test('official TypeScript 7 compiler and native API adapter stay on the same engine', () => {
  assert.equal(nativePackage.name, 'typescript')
  assert.equal(nativePackage.version.split('.')[0], '7')
  assert.equal(bridgePackage.name, 'typescript-native-bridge')
  assert.ok(bridgePackage.version.endsWith(`.tsgo.${nativePackage.version}`))
  const cli = join(dirname(require.resolve('typescript/package.json')), 'bin/tsc')
  const result = spawnSync(process.execPath, [cli, '--version'], { encoding: 'utf8', timeout: 30_000 })
  assert.ifError(result.error)
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), `Version ${nativePackage.version}`)
})

test('native Vue checker accepts valid scripts, templates and component props', () => {
  const result = checkVue('tests/fixtures/typecheck/tsconfig.valid.json')
  assert.equal(result.status, 0, result.output)
})

test('native Vue checker rejects script, template and component prop type errors', () => {
  const result = checkVue('tests/fixtures/typecheck/tsconfig.json')
  assert.notEqual(result.status, 0, result.output)
  assert.match(result.output, /ScriptError\.vue[^\r\n]*TS2322/)
  assert.match(result.output, /TemplateError\.vue[^\r\n]*TS2339/)
  assert.match(result.output, /PropsError\.vue[^\r\n]*TS2322/)
  assert.doesNotMatch(result.output, /Valid(?:Usage)?\.vue[^\r\n]*error TS/)
})
