# strauss-rs 基準測試報告

> **繁體中文** | [English](BENCHMARK.md) | [Benchmark](BENCHMARK.zh-TW.md)

## 測試環境

- **OS**: Windows 11 Home（PHP Strauss 透過 WSL2 Ubuntu 執行）
- **CPU**: AMD/Intel（待測系統）
- **Rust**: 1.94.0 (stable-x86_64-pc-windows-gnu), release build with LTO
- **PHP**: 8.3.30 (WSL2 Ubuntu)
- **PHP Strauss**: [BrianHenryIE/strauss](https://github.com/BrianHenryIE/strauss)（最新 master）

## 測試專案

| 規模 | 依賴 | 總檔案數 | PHP 檔案數 |
|------|------|---------|-----------|
| **小型** | monolog | 147 | 130 |
| **中型** | monolog + guzzlehttp/guzzle + symfony/console | 494 | 410 |
| **大型** | google/apiclient | 33,445 | 33,364 |

## End-to-End 結果

以**真正的 PHP Strauss**（`php bin/strauss dependencies`）進行基準測試。同一台機器、同一個原生 Linux 檔案系統、相同的 vendor 依賴。

| 專案 | PHP 檔案數 | PHP Strauss | Rust strauss-rs | 加速倍率 |
|------|-----------|-------------|-----------------|---------|
| **小型** | 130 | 7,687 ms | 422 ms | **18.2x** |
| **中型** | 410 | 17,908 ms | 1,515 ms | **11.8x** |
| **大型** | 33,364 | 1,528,770 ms (25.5 分鐘) | 123,934 ms (2.1 分鐘) | **12.3x** |

> 每種規模執行 3 次取平均值。大型因 PHP Strauss 耗時 25+ 分鐘僅執行 1 次。兩個工具都在原生 Linux 檔案系統上執行，排除跨 mount I/O 開銷。

## 詳細執行數據

### 小型 (monolog)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 7,710 ms (139 output files) | 426 ms (136 output files) |
| 2 | 7,667 ms | 427 ms |
| 3 | 7,685 ms | 414 ms |
| **平均** | **7,687 ms** | **422 ms** |

### 中型 (monolog + guzzle + symfony/console)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 17,839 ms (353 output files) | 1,647 ms (349 output files) |
| 2 | 17,946 ms | 1,428 ms |
| 3 | 17,939 ms | 1,470 ms |
| **平均** | **17,908 ms** | **1,515 ms** |

### 大型 (google/apiclient)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 1,528,770 ms (33,374 output files) | 123,934 ms (33,019 output files) |

## Phase 分解（Rust strauss-rs, Windows 原生）

在 Windows 上以 `--info` 日誌測量。與上方 WSL 對比數據分開。

### 中型（410 個 PHP 檔案, 約 100 個 namespace）

| Phase | 耗時 | 說明 |
|-------|------|------|
| Phase 1: DISCOVER | ~5.5 ms | 依賴解析、檔案列舉 |
| Phase 2: ANALYZE | ~49 ms | tree-sitter 解析 + symbol 提取 + reference 提取 |
| Phase 3: TRANSFORM | ~68 ms | Per-file 過濾替換 + 寫入 |
| **總計** | **~128 ms** | |

### 大型（33,364 個 PHP 檔案, 28,465 個 symbols）

| Phase | 耗時 | 說明 |
|-------|------|------|
| Phase 1: DISCOVER | ~119 ms | 依賴解析、檔案列舉 |
| Phase 2: ANALYZE | ~22.9 s | tree-sitter 解析 + symbol 提取 + reference 提取 |
| Phase 3: TRANSFORM | ~4.6 s | Per-file 過濾替換 + 寫入 |
| **總計** | **~27.6 s** | |

## Per-File Reference Filtering（Phase 3 優化）

最重要的單一優化。不再讓每個檔案掃描所有 symbols，而是只比對該檔案實際引用的 symbols。

| 專案 | Symbols 數 | 未過濾 | 過濾後 | 加速倍率 |
|------|-----------|--------|--------|---------|
| 小型 | ~30 | 33 ms | 31 ms | ~1x |
| 中型 | ~100 | 270 ms | 68 ms | **4x** |
| **大型** | **28,465** | **1,518,000 ms (25 分鐘)** | **4,600 ms** | **330x** |

運作方式：
- Phase 2 tree-sitter 解析時，同步走訪每個檔案的 AST 收集它引用了哪些 symbols
- Phase 3 為每個檔案建立微型 Aho-Corasick automaton（約 20 個 pattern），而非一個巨大的 automaton（57,000 個 pattern）
- 結果：O(files x avg_refs_per_file) 取代 O(files x total_symbols)

## 為什麼 Rust 比較快？

| 優化項目 | PHP Strauss | Rust strauss-rs |
|---------|-------------|-----------------|
| PHP 解析 | nikic/php-parser (~5 ms/file) | tree-sitter (~0.1 ms/file, **50x**) |
| Parser 實例 | 每個檔案 4+ 次，不重用 | 每個執行緒 1 個，重複使用 |
| Regex 編譯 | 每檔案、每 namespace 重新編譯 | 預編譯一次，全域共用 |
| 檔案 I/O 遍數 | 5+ 次順序遍歷 | 單遍 copy+prefix |
| 平行化 | 無（完全順序） | Rayon 平行處理所有 CPU 核心 |
| Symbol 比對 | 每個檔案掃描所有 symbols | Per-file reference filtering |
| 字串替換 | 每 namespace 個別 regex + str_replace | Aho-Corasick 單遍 O(n) |
| Constant 替換 | 逐一 str_replace | Aho-Corasick 批次替換 |

## 輸出驗證

在中型專案上驗證正確輸出：
- `Monolog\Logger` -> `Benchmark\StraussTest\Deps\Monolog\Logger`
- `GuzzleHttp\Client` -> `Benchmark\StraussTest\Deps\GuzzleHttp\Client`
- `Psr\Http\Message` -> `Benchmark\StraussTest\Deps\Psr\Http\Message`
- 所有 `use` 陳述式正確更新
- Autoloader 檔案已產生
- License 檔案已複製

## 執行檔大小

| | 大小 |
|---|---|
| strauss-rs release binary | **3.9 MB** |
| PHP Strauss（需要 PHP runtime） | ~50 MB (PHP) + ~5 MB (vendor) |
