use anyhow::Result;
use clap::{Parser, Subcommand};
use log::LevelFilter;
use std::path::PathBuf;

mod config;
mod helpers;
mod parser;
mod pipeline;
mod replacer;
mod types;

use pipeline::Pipeline;

#[derive(Parser)]
#[command(name = "strauss", version, about = "High-performance PHP namespace prefixer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Update call sites in project files (true, false, or comma-separated paths)
    #[arg(long = "updateCallSites")]
    update_call_sites: Option<String>,

    /// Delete vendor packages after prefixing
    #[arg(long = "deleteVendorPackages")]
    delete_vendor_packages: Option<bool>,

    /// Dry run - show what would be done without making changes
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Log level
    #[arg(long, default_value = "notice")]
    log_level: Option<String>,

    /// Silent mode
    #[arg(long)]
    silent: bool,

    /// Info level logging
    #[arg(long)]
    info: bool,

    /// Debug level logging
    #[arg(long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Default: process dependencies (copy + prefix)
    Dependencies,
    /// Replace namespaces in-place
    Replace {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        paths: Option<String>,
    },
    /// Add include line to vendor/autoload.php
    IncludeAutoloader,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine log level
    let level = if cli.silent {
        LevelFilter::Off
    } else if cli.debug {
        LevelFilter::Debug
    } else if cli.info {
        LevelFilter::Info
    } else {
        LevelFilter::Warn // "notice" level maps to Warn
    };

    env_logger::Builder::new().filter_level(level).init();

    let working_dir = std::env::current_dir()?;

    match cli.command {
        Some(Commands::Replace { from, to, paths }) => {
            let path_list = paths.map(|p| {
                p.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>()
            });
            pipeline::replace::run_replace(&working_dir, &from, &to, path_list.as_deref())?;
        }
        Some(Commands::IncludeAutoloader) => {
            pipeline::include_autoloader::run_include_autoloader(&working_dir)?;
        }
        // Default command: dependencies
        Some(Commands::Dependencies) | None => {
            let mut pipeline = Pipeline::new(working_dir)?;

            // Apply CLI overrides
            if let Some(ref ucs) = cli.update_call_sites {
                pipeline.config_mut().set_update_call_sites_from_string(ucs);
            }
            if let Some(dvp) = cli.delete_vendor_packages {
                pipeline.config_mut().set_delete_vendor_packages(dvp);
            }
            if cli.dry_run {
                pipeline.config_mut().set_dry_run(true);
            }

            pipeline.run()?;
        }
    }

    Ok(())
}
