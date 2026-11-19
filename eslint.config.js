import js from '@eslint/js'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import pluginVue from 'eslint-plugin-vue'
import { defineConfig } from 'eslint/config'

// typescript-eslint cannot type-lint JSX embedded in a .vue file (upstream #9934).
// Keep the exception finite; new render helpers belong in normal .tsx modules.
const legacyTsxViews = [
  'src/dialogs/LogDialog.vue',
  'src/dialogs/SettingsDialog/components/FmtSettings.vue',
  'src/dialogs/SettingsDialog/components/PluginSettings.vue',
  'src/panes/DownloadPane/components/CompletedProgresses.vue',
  'src/panes/DownloadPane/components/DownloadProgress.vue',
  'src/panes/DownloadPane/components/UncompletedProgresses.vue',
  'src/panes/SearchPane/components/EpisodeCard.vue',
]

export default defineConfig([
  { files: ['**/*.{js,mjs,cjs,ts,mts,cts,vue}'], plugins: { js }, extends: ['js/recommended'] },
  { files: ['**/*.{js,mjs,cjs,ts,mts,cts,vue}'], languageOptions: { globals: globals.browser } },
  tseslint.configs.recommended,
  pluginVue.configs['flat/essential'],
  {
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
  },
  {
    ignores: ['src/bindings.ts', 'src/vite-env.d.ts'],
  },
  {
    files: ['src/**/*.{ts,tsx,vue}'],
    languageOptions: {
      parserOptions: {
        projectService: true,
        extraFileExtensions: ['.vue'],
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/no-floating-promises': ['error', { ignoreVoid: false }],
      '@typescript-eslint/no-misused-promises': ['error', { checksVoidReturn: { attributes: false } }],
    },
  },
  {
    files: ['src/store.ts', 'src/types/**/*.ts', 'src/features/downloads/{events,progress,session,types}.ts'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['*.vue', '**/*.vue'],
              message: 'Shared state and domain logic must not depend on view components.',
            },
          ],
        },
      ],
    },
  },
  {
    files: ['src/**/*.vue'],
    rules: { 'vue/block-lang': ['error', { script: { lang: 'ts' } }] },
  },
  {
    files: legacyTsxViews,
    languageOptions: { parserOptions: { projectService: false, project: false } },
    rules: {
      '@typescript-eslint/no-floating-promises': 'off',
      '@typescript-eslint/no-misused-promises': 'off',
      'vue/block-lang': ['error', { script: { lang: 'tsx' } }],
    },
  },
  {
    files: ['src/features/downloads/{events,progress,session,types}.ts'],
    rules: {
      'no-restricted-imports': [
        'error',
        {
          paths: ['vue', 'pinia'],
          patterns: [
            {
              group: ['*.vue', '**/*.vue', '@tauri-apps/*'],
              message: 'Keep runtime integrations in useDownloadEvents.ts.',
            },
          ],
        },
      ],
    },
  },
])
