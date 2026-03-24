# strauss-rs Benchmark Report

> [繁體中文](BENCHMARK.zh-TW.md) | [English](README.md) | **Benchmark**

## Environment

- **OS**: Windows 11 Home (WSL2 Ubuntu for PHP Strauss)
- **CPU**: AMD/Intel (system under test)
- **Rust**: 1.94.0 (stable-x86_64-pc-windows-gnu), release build with LTO
- **PHP**: 8.3.30 (WSL2 Ubuntu)
- **PHP Strauss**: [BrianHenryIE/strauss](https://github.com/BrianHenryIE/strauss) (latest master)

## Test Projects

| Size | Dependencies | Total files | PHP files |
|------|-------------|------------|-----------|
| **Small** | monolog | 147 | 130 |
| **Medium** | monolog + guzzlehttp/guzzle + symfony/console | 494 | 410 |
| **Large** | google/apiclient | 33,445 | 33,364 |

## End-to-End Results

Benchmarked against the **real PHP Strauss** (`php bin/strauss dependencies`) on the same machine, same native Linux filesystem, same vendor dependencies.

| Project | PHP files | PHP Strauss | Rust strauss-rs | Speedup |
|---------|-----------|-------------|-----------------|---------|
| **Small** | 130 | 7,687 ms | 422 ms | **18.2x** |
| **Medium** | 410 | 17,908 ms | 1,515 ms | **11.8x** |
| **Large** | 33,364 | 1,528,770 ms (25.5 min) | 123,934 ms (2.1 min) | **12.3x** |

> Each size was run 3 times with averages shown. Large was run once due to PHP Strauss taking 25+ minutes. Both tools ran on the native Linux filesystem to eliminate cross-mount I/O overhead.

## Detailed Run Data

### Small (monolog)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 7,710 ms (139 output files) | 426 ms (136 output files) |
| 2 | 7,667 ms | 427 ms |
| 3 | 7,685 ms | 414 ms |
| **Avg** | **7,687 ms** | **422 ms** |

### Medium (monolog + guzzle + symfony/console)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 17,839 ms (353 output files) | 1,647 ms (349 output files) |
| 2 | 17,946 ms | 1,428 ms |
| 3 | 17,939 ms | 1,470 ms |
| **Avg** | **17,908 ms** | **1,515 ms** |

### Large (google/apiclient)

| Run | PHP Strauss | Rust strauss-rs |
|-----|-------------|-----------------|
| 1 | 1,528,770 ms (33,374 output files) | 123,934 ms (33,019 output files) |

## Phase Breakdown (Rust strauss-rs, Windows native)

Measured on Windows with `--info` logging. Separate from the WSL comparison above.

### Medium (410 PHP files, ~100 namespaces)

| Phase | Time | Description |
|-------|------|-------------|
| Phase 1: DISCOVER | ~5.5 ms | Dependency resolution, file enumeration |
| Phase 2: ANALYZE | ~49 ms | tree-sitter parsing + symbol extraction + reference extraction |
| Phase 3: TRANSFORM | ~68 ms | Per-file filtered replacement + write |
| **Total** | **~128 ms** | |

### Large (33,364 PHP files, 28,465 symbols)

| Phase | Time | Description |
|-------|------|-------------|
| Phase 1: DISCOVER | ~119 ms | Dependency resolution, file enumeration |
| Phase 2: ANALYZE | ~22.9 s | tree-sitter parsing + symbol extraction + reference extraction |
| Phase 3: TRANSFORM | ~4.6 s | Per-file filtered replacement + write |
| **Total** | **~27.6 s** | |

## Per-File Reference Filtering (Phase 3 optimization)

The single biggest optimization. Instead of scanning every file against ALL symbols, each file is only matched against symbols it actually references.

| Project | Symbols | Without filtering | With filtering | Speedup |
|---------|---------|-------------------|----------------|---------|
| Small | ~30 | 33 ms | 31 ms | ~1x |
| Medium | ~100 | 270 ms | 68 ms | **4x** |
| **Large** | **28,465** | **1,518,000 ms (25 min)** | **4,600 ms** | **330x** |

How it works:
- During Phase 2 tree-sitter parsing, each file's AST is walked to collect which symbols it references
- Phase 3 builds a tiny per-file Aho-Corasick automaton (~20 patterns) instead of one monolithic automaton (57,000 patterns)
- Result: O(files x avg_refs_per_file) instead of O(files x total_symbols)

## Why is Rust Faster?

| Optimization | PHP Strauss | Rust strauss-rs |
|-------------|-------------|-----------------|
| PHP parsing | nikic/php-parser (~5 ms/file) | tree-sitter (~0.1 ms/file, **50x**) |
| Parser instances | 4+ per file, no reuse | 1 per thread, reused |
| Regex compilation | Per-file, per-namespace | Pre-compiled once, shared |
| File I/O passes | 5+ sequential passes | Single-pass copy+prefix |
| Parallelization | None (sequential) | Rayon parallel across all CPU cores |
| Symbol matching | All symbols scanned per file | Per-file reference filtering |
| String replacement | Per-namespace regex + str_replace | Aho-Corasick single-pass O(n) |
| Constant replacement | Sequential str_replace per constant | Aho-Corasick batch replacement |

## Output Verification

Verified correct output on medium project:
- `Monolog\Logger` -> `Benchmark\StraussTest\Deps\Monolog\Logger`
- `GuzzleHttp\Client` -> `Benchmark\StraussTest\Deps\GuzzleHttp\Client`
- `Psr\Http\Message` -> `Benchmark\StraussTest\Deps\Psr\Http\Message`
- All `use` statements correctly updated
- Autoloader files generated
- License files copied

## Binary Size

| | Size |
|---|---|
| strauss-rs release binary | **3.9 MB** |
| PHP Strauss (requires PHP runtime) | ~50 MB (PHP) + ~5 MB (vendor) |
