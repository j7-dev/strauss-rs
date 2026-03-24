# strauss-rs

> [繁體中文](README.zh-TW.md) | **English** | [Benchmark](BENCHMARK.md)

High-performance PHP namespace prefixer written in Rust. Drop-in replacement for [BrianHenryIE/strauss](https://github.com/BrianHenryIE/strauss).

Reads `composer.json` configuration, copies vendor dependencies to a target directory, and prefixes their PHP namespaces, classes, functions, and constants to avoid conflicts between WordPress plugins (or any PHP project sharing the same runtime).

## Performance

Benchmarked against the **real PHP Strauss** (`BrianHenryIE/strauss` via `php bin/strauss dependencies`) on the same machine, same Linux filesystem, same vendor dependencies.

| Project | Dependencies | PHP files | PHP Strauss | Rust strauss-rs | Speedup |
|---------|-------------|-----------|-------------|-----------------|---------|
| **Small** | monolog | 130 | 7,687 ms | 422 ms | **18.2x** |
| **Medium** | monolog + guzzle + symfony/console | 410 | 17,908 ms | 1,515 ms | **11.8x** |
| **Large** | google/apiclient | 33,364 | 25.5 min | 2.1 min | **12.3x** |

> **Environment:** WSL2 Ubuntu on Windows 11, PHP 8.3.30, Rust release build (`--release`, LTO enabled). Both tools ran on the native Linux filesystem to eliminate cross-mount I/O overhead. Each size was run 3 times; averages shown (Large ran once due to PHP Strauss taking 25+ minutes).

### Why is it faster?

| Optimization | PHP Strauss | Rust strauss-rs |
|-------------|-------------|-----------------|
| PHP parsing | nikic/php-parser (~5ms/file) | tree-sitter (~0.1ms/file, **50x**) |
| Parser instances | 4+ per file, no reuse | 1 per thread, reused |
| Regex compilation | Per-file, per-namespace | Pre-compiled once, shared |
| File I/O passes | 5+ sequential passes | Single-pass copy+prefix |
| Parallelization | None (sequential) | Rayon parallel across all CPU cores |
| Symbol matching | All symbols scanned per file | Per-file reference filtering (only relevant symbols) |
| String replacement | Per-namespace regex + str_replace | Aho-Corasick single-pass O(n) |

### Per-File Reference Filtering

The key optimization for large projects. During parsing, each file's AST is walked to collect which symbols it actually references. In the transform phase, only those symbols are used for replacement — not the entire symbol table.

For google/apiclient (28,465 symbols, 33,363 files):
- **Without filtering:** 57,000-pattern Aho-Corasick per file → 25 minutes for Phase 3
- **With filtering:** ~20-pattern mini-automaton per file → 4.6 seconds (**330x faster**)

## Installation

### From source (requires Rust toolchain)

```bash
git clone https://github.com/j7-dev/strauss-rs.git
cd strauss-rs
cargo build --release
# Binary at: target/release/strauss (or strauss.exe on Windows)
```

### Pre-built binaries

Download from the [Releases](https://github.com/j7-dev/strauss-rs/releases) page.

Choose the archive matching your platform:

| Platform | File |
|----------|------|
| Linux (x86_64) | `strauss-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Intel) | `strauss-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
| macOS (Apple Silicon) | `strauss-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| Windows (x86_64) | `strauss-vX.Y.Z-x86_64-pc-windows-msvc.zip` |

#### Linux / macOS

```bash
# Download and extract (example: Linux x86_64 v0.1.0)
tar xzf strauss-v0.1.0-x86_64-unknown-linux-gnu.tar.gz

# Option 1: Move to a directory in your PATH (recommended)
sudo mv strauss /usr/local/bin/
strauss --version

# Option 2: Use directly from the current directory
./strauss --version
```

#### Windows

1. Extract the `.zip` file
2. **Option A** - Add to PATH (recommended):
   - Move `strauss.exe` to a permanent directory (e.g., `C:\Tools\`)
   - Add that directory to your system PATH:
     Settings > System > About > Advanced system settings > Environment Variables > Path > New
   - Open a **new** terminal and run: `strauss --version`
3. **Option B** - Use the full path directly:
   ```powershell
   .\strauss.exe --version
   ```

## Usage

### Quick start

1. Add strauss configuration to your `composer.json`:

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

2. Run `composer install`

3. Run strauss:

```bash
strauss
```

This copies vendor dependencies to `vendor-prefixed/` with all namespaces, classes, functions, and constants prefixed.

### Commands

```bash
# Default: process and prefix dependencies
strauss

# With options
strauss --updateCallSites true --deleteVendorPackages true

# Dry run (show what would change without writing files)
strauss --dry-run

# Manual namespace replacement
strauss replace --from "Vendor\\Package" --to "MyPrefix\\Vendor\\Package"

# Add autoloader include
strauss include-autoloader
```

### CLI Options

| Flag | Description |
|------|-------------|
| `--updateCallSites <VALUE>` | Update call sites in project files. `true`, `false`, or comma-separated paths |
| `--deleteVendorPackages <VALUE>` | Delete vendor packages after prefixing (`true`/`false`) |
| `--dry-run` | Show what would be done without making changes |
| `--silent` | No output |
| `--info` | Info-level logging |
| `--debug` | Debug-level logging |

## Configuration

All configuration lives in `composer.json` under `extra.strauss`. Also supports `extra.mozart` for backward compatibility.

### Required fields

None — all fields have sensible defaults inferred from your `composer.json`.

### Common fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace_prefix` | string | *inferred from PSR-4* | Prefix added to PHP namespaces |
| `classmap_prefix` | string | *inferred from namespace_prefix* | Prefix for global (non-namespaced) classes |
| `target_directory` | string | `"vendor-prefixed"` | Output directory for prefixed files |
| `packages` | string[] | *all require keys* | Which packages to process |

### Exclusion rules

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

### All configuration fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace_prefix` | string | inferred | Namespace prefix for PHP namespaces |
| `classmap_prefix` | string | inferred | Prefix for non-namespaced classes |
| `functions_prefix` | string\|false | inferred | Prefix for PHP functions (`false` to disable) |
| `constants_prefix` | string | inferred | Prefix for PHP constants |
| `target_directory` | string | `"vendor-prefixed"` | Output directory |
| `vendor_directory` | string | `"vendor"` | Vendor directory path |
| `packages` | string[] | require keys | Packages to process |
| `update_call_sites` | bool\|string[] | `false` | Update project call sites |
| `delete_vendor_files` | bool | `false` | Delete processed vendor files |
| `delete_vendor_packages` | bool | `false` | Delete entire vendor packages |
| `classmap_output` | bool | auto | Generate classmap output file |
| `include_modified_date` | bool | `true` | Add modification date to file headers |
| `include_author` | bool | `true` | Add author to file headers |
| `include_root_autoload` | bool | `false` | Include root autoload in generated loader |
| `namespace_replacement_patterns` | object | `{}` | Custom regex replacement patterns |
| `exclude_from_copy` | object | `{}` | Packages/namespaces/patterns to skip copying |
| `exclude_from_prefix` | object | `{}` | Packages/namespaces/patterns to skip prefixing |
| `override_autoload` | object | `{}` | Per-package autoload overrides |

## How it works

### 3-Phase Pipeline

```
Phase 1: DISCOVER (sequential)
  Parse composer.json/lock → resolve dependency tree → enumerate files → apply exclusion rules

Phase 2: ANALYZE (parallel)
  tree-sitter parse all PHP files → extract symbols (namespace, class, function, constant)
  → extract per-file references → mark symbols for renaming → compute replacements

Phase 3: TRANSFORM (parallel)
  For each file: filter symbols to only relevant ones → Aho-Corasick replace → write
  → update project call sites → generate autoloader → cleanup
```

### Per-File Reference Filtering

The key optimization that makes large projects feasible. During Phase 2, each file's AST is walked to collect which symbols it references. In Phase 3, only those symbols are used for replacement — not the entire symbol table.

For google/apiclient (28,465 symbols, 33,363 files):
- **Without filtering:** 57,000-pattern Aho-Corasick automaton per file → 25 minutes
- **With filtering:** ~20-pattern mini-automaton per file → 4.6 seconds (**330x faster**)

## Compatibility

- Reads the same `composer.json extra.strauss` configuration as PHP Strauss
- Supports `extra.mozart` backward compatibility
- Cross-platform: Windows, Linux, macOS

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Build release binary
cargo build --release
```

## License

MIT
