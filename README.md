# strauss-rs

High-performance PHP namespace prefixer written in Rust. Drop-in replacement for [BrianHenryIE/strauss](https://github.com/BrianHenryIE/strauss).

Reads `composer.json` configuration, copies vendor dependencies to a target directory, and prefixes their PHP namespaces, classes, functions, and constants to avoid conflicts between WordPress plugins (or any PHP project sharing the same runtime).

## Performance

Benchmarked on Windows 11 (same machine, same projects):

| Project | Files | PHP | Rust | Speedup |
|---------|-------|-----|------|---------|
| Small (monolog) | 138 | 128 ms | 44 ms | **2.9x** |
| Medium (monolog + guzzle + symfony) | 419 | 584 ms | 128 ms | **4.6x** |
| Large (google/apiclient) | 33,363 | 45.3 s | 30.2 s | **1.5x** |

> **Note:** The PHP benchmark only replaces namespaces (716 patterns). Rust replaces namespaces + classes + functions + constants (28,465 symbols). Comparing equal workloads, Rust is estimated **5-10x faster**.

### Key optimizations

- **tree-sitter** for PHP parsing (~50x faster than nikic/php-parser)
- **Per-file reference filtering** — each file only matches against symbols it actually uses, not the entire symbol table
- **Aho-Corasick** single-pass multi-pattern replacement (O(n) per file)
- **Rayon** parallel file processing across all CPU cores
- **Single-pass copy+prefix** — no separate copy/prefix/license passes

## Installation

### From source (requires Rust toolchain)

```bash
git clone https://github.com/nicholasgasior/strauss-rs.git
cd strauss-rs
cargo build --release
# Binary at: target/release/strauss (or strauss.exe on Windows)
```

### Pre-built binaries

Download from the [Releases](https://github.com/nicholasgasior/strauss-rs/releases) page.

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
