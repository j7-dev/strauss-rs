# strauss-rs

> **繁體中文** | [English](README.md) | [Benchmark](BENCHMARK.zh-TW.md)

以 Rust 撰寫的高效能 PHP namespace prefixer。可直接替換 [BrianHenryIE/strauss](https://github.com/BrianHenryIE/strauss)。

讀取 `composer.json` 設定，將 vendor 依賴複製到目標目錄，並為所有 PHP namespace、class、function 和 constant 加上前綴，避免 WordPress 外掛（或任何共享 runtime 的 PHP 專案）之間的命名衝突。

## 效能

以**真正的 PHP Strauss**（`BrianHenryIE/strauss`，透過 `php bin/strauss dependencies` 執行）進行基準測試。同一台機器、同一個 Linux 檔案系統、相同的 vendor 依賴。

| 專案 | 依賴 | PHP 檔案數 | PHP Strauss | Rust strauss-rs | 加速倍率 |
|------|------|-----------|-------------|-----------------|---------|
| **小型** | monolog | 130 | 7,687 ms | 422 ms | **18.2x** |
| **中型** | monolog + guzzle + symfony/console | 410 | 17,908 ms | 1,515 ms | **11.8x** |
| **大型** | google/apiclient | 33,364 | 25.5 分鐘 | 2.1 分鐘 | **12.3x** |

> **測試環境：** WSL2 Ubuntu on Windows 11, PHP 8.3.30, Rust release build (`--release`, LTO enabled)。兩個工具都在原生 Linux 檔案系統上執行，排除跨 mount I/O 開銷。每種規模執行 3 次取平均值（大型因 PHP Strauss 耗時 25+ 分鐘僅執行 1 次）。

### 為什麼比較快？

| 優化項目 | PHP Strauss | Rust strauss-rs |
|---------|-------------|-----------------|
| PHP 解析 | nikic/php-parser (~5ms/file) | tree-sitter (~0.1ms/file, **50x**) |
| Parser 實例 | 每個檔案 4+ 次，不重用 | 每個執行緒 1 個，重複使用 |
| Regex 編譯 | 每檔案、每 namespace 重新編譯 | 預編譯一次，全域共用 |
| 檔案 I/O 遍數 | 5+ 次順序遍歷 | 單遍 copy+prefix |
| 平行化 | 無（完全順序） | Rayon 平行處理所有 CPU 核心 |
| Symbol 比對 | 每個檔案掃描所有 symbols | Per-file reference filtering（僅比對相關 symbols） |
| 字串替換 | 每 namespace 個別 regex + str_replace | Aho-Corasick 單遍 O(n) |

### Per-File Reference Filtering

讓大型專案變得可行的關鍵優化。在解析階段，會走訪每個檔案的 AST 收集它引用了哪些 symbols。在轉換階段，只使用這些 symbols 進行替換——而非整個 symbol table。

以 google/apiclient（28,465 個 symbols, 33,363 個檔案）為例：
- **未過濾：** 每個檔案用 57,000 個 pattern 的 Aho-Corasick → Phase 3 耗時 25 分鐘
- **過濾後：** 每個檔案約 20 個 pattern 的 mini-automaton → 4.6 秒（**快 330 倍**）

## 安裝

### 從原始碼編譯（需要 Rust 工具鏈）

```bash
git clone https://github.com/nicholasgasior/strauss-rs.git
cd strauss-rs
cargo build --release
# 執行檔位於：target/release/strauss（Windows 為 strauss.exe）
```

### 預編譯二進位檔

從 [Releases](https://github.com/nicholasgasior/strauss-rs/releases) 頁面下載。

## 使用方式

### 快速開始

1. 在 `composer.json` 中加入 strauss 設定：

```json
{
    "name": "my/wordpress-plugin",
    "autoload": {
        "psr-4": {
            "MyPlugin\\": "src/"
        }
    },
    "require": {
        "monolog/monolog": "^3.0",
        "guzzlehttp/guzzle": "^7.0"
    },
    "extra": {
        "strauss": {
            "target_directory": "vendor-prefixed",
            "namespace_prefix": "MyPlugin\\Deps\\",
            "classmap_prefix": "MyPlugin_Deps_"
        }
    }
}
```

2. 執行 `composer install`

3. 執行 strauss：

```bash
strauss
```

這會將 vendor 依賴複製到 `vendor-prefixed/`，並為所有 namespace、class、function 和 constant 加上前綴。

### 指令

```bash
# 預設：處理並前綴化依賴
strauss

# 附帶選項
strauss --updateCallSites true --deleteVendorPackages true

# 試執行（顯示會做什麼但不實際寫入檔案）
strauss --dry-run

# 手動 namespace 替換
strauss replace --from "Vendor\\Package" --to "MyPrefix\\Vendor\\Package"

# 加入 autoloader include
strauss include-autoloader
```

### CLI 選項

| 旗標 | 說明 |
|------|------|
| `--updateCallSites <VALUE>` | 更新專案檔案中的呼叫位置。`true`、`false` 或逗號分隔的路徑 |
| `--deleteVendorPackages <VALUE>` | 前綴化後刪除 vendor 套件（`true`/`false`） |
| `--dry-run` | 顯示會做什麼但不實際執行 |
| `--silent` | 靜默模式 |
| `--info` | Info 層級日誌 |
| `--debug` | Debug 層級日誌 |

## 設定

所有設定位於 `composer.json` 的 `extra.strauss` 區塊。也支援 `extra.mozart` 向下相容。

### 必填欄位

無 — 所有欄位都有從 `composer.json` 推斷的合理預設值。

### 常用欄位

| 欄位 | 型別 | 預設值 | 說明 |
|------|------|--------|------|
| `namespace_prefix` | string | *從 PSR-4 推斷* | 加到 PHP namespace 的前綴 |
| `classmap_prefix` | string | *從 namespace_prefix 推斷* | 全域 class 的前綴 |
| `target_directory` | string | `"vendor-prefixed"` | 輸出目錄 |
| `packages` | string[] | *所有 require keys* | 要處理的套件 |

### 排除規則

```json
{
    "extra": {
        "strauss": {
            "exclude_from_copy": {
                "packages": ["symfony/polyfill-php80"],
                "namespaces": ["Symfony\\Polyfill"],
                "file_patterns": ["/\\.md$/"]
            },
            "exclude_from_prefix": {
                "packages": ["psr/log"],
                "namespaces": ["Psr\\Log"]
            }
        }
    }
}
```

### 所有設定欄位

| 欄位 | 型別 | 預設值 | 說明 |
|------|------|--------|------|
| `namespace_prefix` | string | 推斷 | PHP namespace 前綴 |
| `classmap_prefix` | string | 推斷 | 非 namespace class 的前綴 |
| `functions_prefix` | string\|false | 推斷 | PHP function 前綴（`false` 停用） |
| `constants_prefix` | string | 推斷 | PHP constant 前綴 |
| `target_directory` | string | `"vendor-prefixed"` | 輸出目錄 |
| `vendor_directory` | string | `"vendor"` | Vendor 目錄路徑 |
| `packages` | string[] | require keys | 要處理的套件 |
| `update_call_sites` | bool\|string[] | `false` | 更新專案呼叫位置 |
| `delete_vendor_files` | bool | `false` | 刪除已處理的 vendor 檔案 |
| `delete_vendor_packages` | bool | `false` | 刪除整個 vendor 套件 |
| `classmap_output` | bool | 自動 | 產生 classmap 輸出檔 |
| `include_modified_date` | bool | `true` | 在檔頭加入修改日期 |
| `include_author` | bool | `true` | 在檔頭加入作者 |
| `include_root_autoload` | bool | `false` | 在產生的 loader 中包含根 autoload |
| `namespace_replacement_patterns` | object | `{}` | 自訂 regex 替換模式 |
| `exclude_from_copy` | object | `{}` | 跳過複製的套件/namespace/模式 |
| `exclude_from_prefix` | object | `{}` | 跳過前綴化的套件/namespace/模式 |
| `override_autoload` | object | `{}` | 各套件的 autoload 覆寫 |

## 運作原理

### 三階段 Pipeline

```
Phase 1: DISCOVER（循序）
  解析 composer.json/lock → 解析依賴樹 → 列舉檔案 → 套用排除規則

Phase 2: ANALYZE（平行）
  tree-sitter 解析所有 PHP 檔案 → 提取 symbols（namespace, class, function, constant）
  → 提取 per-file references → 標記待重新命名的 symbols → 計算替換名稱

Phase 3: TRANSFORM（平行）
  每個檔案：過濾出相關 symbols → Aho-Corasick 替換 → 寫入
  → 更新專案呼叫位置 → 產生 autoloader → 清理
```

### Per-File Reference Filtering

讓大型專案變得可行的關鍵優化。在 Phase 2 中，會走訪每個檔案的 AST 收集它引用了哪些 symbols。在 Phase 3 中，只使用這些 symbols 進行替換——而非整個 symbol table。

以 google/apiclient（28,465 個 symbols, 33,363 個檔案）為例：
- **未過濾：** 每個檔案用 57,000 個 pattern 的 Aho-Corasick automaton → 25 分鐘
- **過濾後：** 每個檔案約 20 個 pattern 的 mini-automaton → 4.6 秒（**快 330 倍**）

## 相容性

- 讀取與 PHP Strauss 相同的 `composer.json extra.strauss` 設定格式
- 支援 `extra.mozart` 向下相容
- 跨平台：Windows、Linux、macOS

## 開發

```bash
# 執行測試
cargo test

# 以 debug 日誌執行
RUST_LOG=debug cargo run

# 編譯 release 版本
cargo build --release
```

## 授權

MIT
