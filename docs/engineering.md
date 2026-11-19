# 架構、效能與 CI 規則

這份文件把使用者的「先質疑、刪除、簡化、縮短週期，最後自動化」落實為少數可檢查的規則。規則由專案中的失敗模式推導；論文提供設計與量測方法，並不替本專案保證效能。

## 研究依據與本專案的取捨

| 研究                                                                                                                                                                                              | 採用的方法                                                             | 在本專案的落點                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Parnas，1972，[On the Criteria To Be Used in Decomposing Systems into Modules](https://doi.org/10.1145/361598.361623)，另見 [CMU 技術報告](https://prl.khoury.northeastern.edu/img/p-tr-1971.pdf) | 依容易改變的設計決策劃分模組，隱藏內部表示；分層本身也可能增加執行成本 | 把 API envelope、檔案替換、下載排程及前端事件規則隔離；保留原有命令介面，使用直接函式與泛型而非新增動態分派層 |
| Mytkowicz 等，2009，[Producing Wrong Data Without Doing Anything Obviously Wrong!](https://evaluate.inf.usi.ch/node/4)                                                                            | 實驗設定會造成量測偏差，需檢查比較順序及環境因素                       | 基準使用同一份資料，交替新舊實作的執行順序；區分 CPU 解碼和網路、磁碟、GUI 工作，不把結果外推到下載吞吐量     |
| Kalibera、Jones，2013，[Rigorous Benchmarking in Reasonable Time](https://kar.kent.ac.uk/33611/)                                                                                                  | 重複量測並呈現不確定性，把實驗成本放在主要變異來源                     | 每個資料量預熱、收集 30 組原始樣本及配對比值區間；正式比較重複啟動程序，避免只看一次最快結果                  |

以上是工程上的應用，不是完整複製論文實驗。微基準的 bootstrap 區間只描述該次程序的樣本，沒有涵蓋不同硬體、編譯器或系統負載的變異。

## 保留下來的模組邊界

- `bili_client.rs` 隱藏 HTTP client 與建立／替換方式；`auth.rs` 管理登入及 Cookie，`content.rs` 管理內容查詢，`media.rs` 管理媒體傳輸。它們保留同一個 `BiliClient` 介面。
- `bili_client/response.rs` 統一 HTTP 狀態、API 錯誤、`data/result` 以及資料解碼規則。直接消耗 JSON Value，移除中間字串及成功路徑上大量的錯誤訊息配置。
- `storage.rs` 是設定、插件 metadata 與任務進度共用的同目錄暫存檔／原子替換實作。`progress_store.rs` 負責任務檔案與刪除的順序；資料模型不操作 Tauri 路徑。
- `downloader/chunk_transfer.rs` 限制存活工作數並傳遞中斷及失敗。`media_download.rs` 共用影音暫存檔與斷點處理；彈幕分段請求也依並行設定限制工作數，子工作異常不再被忽略。
- `features/downloads` 的事件、進度及 session 邏輯可獨立測試；Tauri／Vue 接線留在 composable。`features/search/identifiers.ts` 只負責識別碼解析，拒絕部分有效的數字前綴。

## 八類規則如何落實

| 類別             | 禁止的大宗寫法                                        | 自動檢查或證據                                                         |
| ---------------- | ----------------------------------------------------- | ---------------------------------------------------------------------- |
| 邊界與重複規則   | 共用資料層反向匯入畫面；各 API 自行複製 envelope 規則 | ESLint 匯入限制、API 解碼回歸測試、程式碼審查                          |
| 並行與資源       | 無上限建立工作、漏掉中斷或 panic                      | 分片工作峰值與取消測試                                                 |
| 鎖與阻塞         | 持有同步鎖跨 await、在 runtime worker 上 sleep        | Clippy、移除 parking_lot 的 send_guard、阻塞操作的範圍審查             |
| 失敗處理         | 外部操作 unwrap、丟棄 Promise、遺留 dbg／todo         | Rust unused_must_use、Clippy、TypeScript、型別感知 ESLint              |
| 配置與重複處理   | JSON 字串往返、熱路徑的大量 clone／重複掃描           | CPU 微基準、Clippy perf 群組與審查；不以文字掃描假裝能偵測所有慢演算法 |
| 生命週期與一致性 | 訂閱未解除、刪除後復活、寫壞正式檔案                  | session、分片、儲存與設定回歸測試                                      |
| 原生邊界         | 無 SAFETY 前提的 unsafe、同時操作插件借用緩衝區       | Clippy undocumented_unsafe_blocks、明確 ABI 契約與序列化呼叫           |
| 量測與變更       | 以單次計時宣稱加速、只加抽象卻沒有行為或資料依據      | 基準原始樣本、輸出相等檢查與審查                                       |

規則集中在 `AGENTS.md`、`clippy.toml`、Cargo manifests、`eslint.config.js` 與 `tsconfig.json`。啟動失敗允許有理由的 `expect`；測試允許斷言式 unwrap；產生碼只保留具理由的必要例外。

### 前端工具限制

既有七個 `.vue` 檔在 script 中混用 TSX；上游 [typescript-eslint #9934](https://github.com/typescript-eslint/typescript-eslint/issues/9934) 及 [Vue 官方設定文件](https://github.com/vuejs/eslint-config-typescript#project-options) 記錄了它與型別感知規則的衝突。這七個固定路徑保留語法 ESLint 與全量 `vue-tsc`，不宣稱它們受 Promise 型別規則完整涵蓋；其他 TS／TSX／Vue TS 都啟用相關規則。新增 Vue 元件強制使用 TS script；渲染函式可放在正常 `.tsx` 檔，不擴大例外清單。

## CI 是什麼，以及這裡檢查什麼

CI（持續整合）會在 GitHub 收到變更時，自動執行檢查。本專案使用 GitHub Actions；CI 不屬於單一程式語言。

`CI` workflow 在 PR、main／develop 推送、merge queue 與手動觸發時執行。取消已被新提交取代的舊工作，但不使用路徑過濾讓必要檢查跳過。

1. 前端：Windows／Linux／macOS 上執行鎖檔安裝、格式、ESLint、TypeScript、單元測試與正式建置，涵蓋原生打包工具的各平台依賴。
2. 後端：固定 Rust 1.100.0，Windows／Linux／macOS 上執行零警告 Clippy、核心與插件測試；Linux 另檢查 Rust 格式。
3. `quality-gate`：所有必要工作成功才通過；失敗、取消或跳過都不算成功。

GitHub 的 Actions 開關與「檢查沒過禁止合併」是不同設定。分支保護應要求 `quality-gate` 並要求基底為最新；設定是否已套用需以 GitHub 的實際狀態確認。

2026-11-19 已於本倉庫的 `main` 與 `develop` 套用並讀回確認：必要檢查限定由 GitHub Actions 提供的 `quality-gate`，要求基底為最新，管理員亦受限制，禁止強制推送及刪除分支。重構變更由 [PR #1](https://github.com/crazysmile-PhD/bilibili-video-downloader/pull/1) 驗證；這些遠端設定不會隨 clone 自動套用到其他倉庫。

## 效能量測

```sh
pnpm benchmark
```

`src-tauri/benches/api_decode.rs` 比較舊有 envelope→Value→字串→型別與現行 Value→型別流程，確認輸出相等。輸入含 Unicode，資料量為 20／200／2000 筆。每個資料量預熱 20 次、交替比較 30 組；輸出包含所有樣本、中位數、配對加速比與 bootstrap 區間。

使用 `BENCH_OUTPUT` 指定已存在目錄下的絕對輸出路徑；Cargo 執行時的相對路徑以 `src-tauri` 為基準。正式記錄至少重複三個程序，記錄 OS、CPU、Rust 版本、編譯 profile、負載及樣本；避免同時進行其他編譯。這不是整體下載速度測試。

一般 PR 不以短時間計時作為硬門檻；確定性的工作數上限、取消、內容一致性與型別規則是硬門檻。手動 CI 可選擇執行基準並保存 artifact；評估穩定基準後，再設定可說明的效能預算。

本機三次程序的結果與完整樣本見 [2026-11-19 量測記錄](benchmarks/2026-11-19/README.md)。

## 依賴更新與相容性

2026-11-19 更新前端、三個 Cargo 鎖檔，以及 CI、發布與 issue 處理使用的 Actions。前端採 Node.js 24 以上與 packageManager 固定的 pnpm 12.21.0；CI 與發布使用相同的 Node／Rust 版本及 frozen lockfile。

- Vite 8、Vue 3.5、Pinia 4、ESLint 10、vue-tsc 3 及其相容套件已更新。明確保留 Vite 6 原有的瀏覽器編譯目標，避免工具升級同時提高 WebView 門檻。
- 專案直接使用官方 `typescript@7.0.2`。Vue 與 ESLint 需要的 Compiler API 由 `typescript-native-bridge@6.0.3-bridge.19.tsgo.7.0.2` 提供；其中 `6.0.3` 是 JS API 相容層版本，`tsgo.7.0.2` 是實際執行型別判斷的 Go 引擎版本。
- Tauri、HTTP client／middleware／retry、檔案配置、鎖、雜湊及其他 Rust 依賴同步更新。HTTP 的 query feature 改為明確啟用；MD5 升級後以已知向量驗證 WBI 所需的 32 字元小寫十六進位格式。
- Specta／tauri-specta／specta-typescript 保留既有協調版本。此組依賴負責產生前後端命令契約；遷移新版預發行 API 應獨立驗證，不能以較舊的 stable 1.x 取代現有 2.x RC。
- Actions 使用完整 commit SHA，旁註已驗證的版本。更新時同時核對上游 release notes、Action 輸入及 Node runtime；不要只修改註解或放寬合併門檻。
- pnpm 保留未審核建置腳本即失敗的預設，只允許 `vue-demi@0.14.10` 的本機 Vue 版本橋接腳本，以及 `fsevents@2.3.3` 在 macOS 以 node-gyp 建置檔案監看的原生綁定。前端 CI 擴及三平台，驗證這些安裝及打包差異。`minimumReleaseAgeExclude` 只記錄本次明確選用的新發布 CLI／virtua 版本，不使用全域停用或萬用字元。

版本依據：[npm 官方 registry](https://registry.npmjs.org/)、[crates.io](https://crates.io/)、[Vite 遷移文件](https://vite.dev/guide/migration)、[pnpm 建置設定](https://pnpm.io/settings/build)，以及各 Action 的官方 releases。合併前須讓更新後的鎖檔、檢查、測試及建置重新通過；版本更新本身不構成效能改善證據。

### TypeScript 7 的檢查功能

直接安裝官方 7.0.2 時，已實際重現 `vue-tsc` 找不到 `typescript/lib/tsc` 的失敗。改用 Vue 維護者提供的 [原生 API 橋接器](https://github.com/johnsoncodehk/typescript-native-bridge)，讓舊 API 工具使用 7.0.2 型別引擎；此路徑亦見 [Vue 維護者的支援說明](https://github.com/vuejs/language-tools/issues/5381)。

`.pnpmfile.cjs` 只調整 typescript-eslint 8、Vue ESLint 10、vue-tsc 3 與 Volar TypeScript 2 的工具依賴；根目錄官方 TypeScript 7 與 CLI 維持獨立。這些工具的 classic API peer 改為明確依賴原生橋接器，沒有修改工具的原始碼或關閉 lint 規則。更新工具主要版本時需重新驗證此邊界。

工具選擇以實際攔錯能力為準。比較中的 Oxlint 1.94.0／oxlint-tsgolint 7.0.2007 在同一個漏接 Promise 案例只回報 TS 檔，未回報 Vue SFC，因而未用它取代現有 Vue 防線。已封存的 Vue-tsgo 原型亦未加入依賴。保留的方案通過全專案檢查，並新增以下反例驗收：

- 官方 CLI 回報 TypeScript 7，API 橋接器綁定同一版 Go 引擎，Vue 檢查過程須回報原生橋接已啟用。
- 正確的 Vue script、模板與元件 props 通過；故意錯誤的賦值、模板方法及 props 型別必須失敗。
- 純 TS 與 Vue script 中未處理的 Promise、傳給同步 callback 的 Promise 必須被 lint 擋下；純邏輯匯入畫面仍須失敗。

這些測試與既有型別、格式、建置及三平台 CI 一同執行。原生引擎載入失敗會讓檢查失敗，沒有退回 JS 型別檢查器的分支；既有七個 TSX SFC 的有限例外亦未擴大。
