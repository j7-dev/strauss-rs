pub mod dependencies;
pub mod file_enumerator;
pub mod autoloaded_files;
pub mod file_copy_scanner;
pub mod symbol_scanner;
pub mod mark_symbols;
pub mod change_enumerator;
pub mod copier;
pub mod prefixer;
pub mod licenser;
pub mod autoload;
pub mod cleanup;
pub mod aliases;
pub mod replace;
pub mod include_autoloader;

use anyhow::Result;
use log::{debug, info, warn};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config::strauss_config::StraussConfig;
use crate::config::composer_package::ComposerPackage;
use crate::types::discovered_files::DiscoveredFiles;
use crate::types::symbols_collection::DiscoveredSymbols;
use crate::replacer::Replacer;

/// The main pipeline orchestrator.
/// Implements the 3-phase architecture for maximum performance.
pub struct Pipeline {
    working_dir: PathBuf,
    config: StraussConfig,
    dependency_tree: Vec<ComposerPackage>,
    discovered_files: DiscoveredFiles,
    discovered_symbols: DiscoveredSymbols,
}

impl Pipeline {
    /// Create a new pipeline from the working directory.
    /// Reads composer.json and initializes config.
    pub fn new(working_dir: PathBuf) -> Result<Self> {
        let composer_json_path = working_dir.join("composer.json");
        let config = StraussConfig::from_composer_json(&composer_json_path)?;

        Ok(Self {
            working_dir,
            config,
            dependency_tree: Vec::new(),
            discovered_files: DiscoveredFiles::new(),
            discovered_symbols: DiscoveredSymbols::new(),
        })
    }

    /// Get mutable reference to config for CLI overrides.
    pub fn config_mut(&mut self) -> &mut StraussConfig {
        &mut self.config
    }

    /// Run the full pipeline.
    ///
    /// Phase 1: DISCOVER (sequential, fast I/O)
    /// Phase 2: ANALYZE (parallel, CPU-bound)
    /// Phase 3: TRANSFORM (parallel, I/O + CPU)
    pub fn run(&mut self) -> Result<()> {
        let total_start = Instant::now();

        log::warn!("Starting strauss-rs...");

        // ═══════════════════════════════════════════
        // Phase 1: DISCOVER
        // ═══════════════════════════════════════════
        let phase1_start = Instant::now();

        // 1a. Build dependency tree
        self.dependency_tree = dependencies::enumerate_dependencies(
            &self.working_dir,
            &self.config,
        )?;
        info!("Found {} dependencies", self.dependency_tree.len());

        // 1b. Enumerate all files
        self.discovered_files = file_enumerator::enumerate_files(
            &self.working_dir,
            &self.config,
            &self.dependency_tree,
        )?;
        info!("Found {} files", self.discovered_files.len());

        // 1c. Map autoloaded files
        autoloaded_files::enumerate_autoloaded_files(
            &mut self.discovered_files,
            &self.dependency_tree,
        )?;

        // 1d. Apply copy/prefix exclusion rules
        file_copy_scanner::scan_files(
            &mut self.discovered_files,
            &self.config,
        )?;

        let phase1_time = phase1_start.elapsed();
        info!("Phase 1 (DISCOVER) completed in {:?}", phase1_time);

        // ═══════════════════════════════════════════
        // Phase 2: ANALYZE (parallel)
        // ═══════════════════════════════════════════
        let phase2_start = Instant::now();

        // 2a. Parse all PHP files in parallel, extract symbols + per-file references
        let (symbols, file_refs) = symbol_scanner::scan_files_for_symbols(
            &self.discovered_files,
            &self.config,
        )?;
        self.discovered_symbols = symbols;
        info!("Found {} symbols", self.discovered_symbols.len());

        // Merge per-file references into DiscoveredFiles
        let refs_count = file_refs.len();
        for entry in file_refs.into_iter() {
            let (path, refs) = entry;
            if let Some(file) = self.discovered_files.get_file_mut(&path) {
                file.file_references = Some(refs);
            }
        }
        info!("Attached references to {} files", refs_count);

        // 2b. Add PSR-4 namespace symbols
        for pkg in &self.dependency_tree {
            for (ns, _paths) in &pkg.autoload_psr4 {
                let ns_trimmed = ns.trim_end_matches('\\').to_string();
                if !ns_trimmed.is_empty() {
                    use crate::types::symbol::{DiscoveredSymbol, SymbolType};
                    self.discovered_symbols.add(DiscoveredSymbol::new_namespace(
                        ns_trimmed,
                        String::new(),
                        Some(pkg.name.clone()),
                    ));
                }
            }
        }

        // 2c. Mark symbols for renaming
        mark_symbols::mark_symbols_for_renaming(
            &mut self.discovered_symbols,
            &self.discovered_files,
            &self.config,
        )?;

        // 2d. Determine replacement names
        change_enumerator::determine_replacements(
            &mut self.discovered_symbols,
            &self.config,
        )?;

        let phase2_time = phase2_start.elapsed();
        info!("Phase 2 (ANALYZE) completed in {:?}", phase2_time);

        // ═══════════════════════════════════════════
        // Phase 3: TRANSFORM (parallel)
        // ═══════════════════════════════════════════
        let phase3_start = Instant::now();

        // Build the replacer engine with compiled patterns
        let mut replacer = Replacer::new(
            self.config.namespace_prefix.clone(),
            self.config.classmap_prefix.clone(),
            self.config.constants_prefix.clone(),
            self.config.get_exclude_namespaces_from_prefixing(),
        );
        // Pre-compute all replacement data ONCE
        replacer.prepare(&self.discovered_symbols);

        if !self.config.dry_run {
            // 3a. Copy files + apply replacements in single pass (parallel)
            copier::copy_and_prefix(
                &self.discovered_files,
                &self.discovered_symbols,
                &self.config,
                &replacer,
            )?;

            // 3b. Update project call sites (if configured)
            if let Some(ref call_sites) = self.config.update_call_sites {
                if !call_sites.is_empty() {
                    prefixer::replace_in_project_files(
                        &self.working_dir,
                        call_sites,
                        &self.discovered_symbols,
                        &replacer,
                    )?;
                }
            }

            // 3c. Add license headers
            licenser::add_licenses(
                &self.discovered_files,
                &self.dependency_tree,
                &self.config,
            )?;

            // 3d. Cleanup
            cleanup::cleanup(
                &self.discovered_files,
                &self.dependency_tree,
                &self.config,
            )?;

            // 3e. Generate autoloader
            autoload::generate_autoloader(
                &self.working_dir,
                &self.discovered_files,
                &self.discovered_symbols,
                &self.dependency_tree,
                &self.config,
            )?;

            // 3f. Generate aliases (if delete_vendor_packages is enabled)
            if self.config.delete_vendor_packages {
                aliases::generate_aliases(
                    &self.working_dir,
                    &self.discovered_symbols,
                    &self.config,
                )?;
            }
        }

        let phase3_time = phase3_start.elapsed();
        info!("Phase 3 (TRANSFORM) completed in {:?}", phase3_time);

        let total_time = total_start.elapsed();
        log::warn!(
            "Done in {:?} (discover: {:?}, analyze: {:?}, transform: {:?})",
            total_time, phase1_time, phase2_time, phase3_time
        );

        Ok(())
    }
}
