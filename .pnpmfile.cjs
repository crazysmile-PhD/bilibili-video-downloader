// TypeScript 7 ships a native compiler, without the classic Compiler API.
// Route only legacy API tools to the Vue maintainer's tsgo 7.0.2 adapter;
// keep the application's official typescript@7.0.2 package and CLI intact.
const nativeApi = 'npm:typescript-native-bridge@6.0.3-bridge.19.tsgo.7.0.2'

module.exports = {
  hooks: {
    readPackage(pkg) {
      const usesClassicApi =
        ((pkg.name === 'typescript-eslint' || pkg.name.startsWith('@typescript-eslint/')) &&
          pkg.version.startsWith('8.')) ||
        (pkg.name === 'vue-tsc' && pkg.version.startsWith('3.')) ||
        (pkg.name === '@volar/typescript' && pkg.version.startsWith('2.')) ||
        (pkg.name === 'eslint-plugin-vue' && pkg.version.startsWith('10.'))
      if (!usesClassicApi) return pkg

      const peers = { ...pkg.peerDependencies }
      delete peers.typescript
      const peerMeta = { ...pkg.peerDependenciesMeta }
      delete peerMeta.typescript
      return {
        ...pkg,
        dependencies: { ...pkg.dependencies, typescript: nativeApi },
        peerDependencies: peers,
        peerDependenciesMeta: peerMeta,
      }
    },
  },
}
