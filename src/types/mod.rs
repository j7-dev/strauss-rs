//! Type definitions for discovered PHP symbols and files.
//!
//! This module contains the core data structures used throughout the Strauss pipeline:
//!
//! - [`DiscoveredSymbol`] and [`SymbolType`] - represents a discovered PHP namespace,
//!   class, interface, trait, function, or constant.
//! - [`DiscoveredSymbols`] - thread-safe concurrent collection of symbols (backed by `DashMap`).
//! - [`DiscoveredFile`] - represents a file discovered in the project or vendor directory.
//! - [`DiscoveredFiles`] - collection of discovered files keyed by source path.

pub mod discovered_files;
pub mod file;
pub mod file_references;
pub mod symbol;
pub mod symbols_collection;

// Re-export primary types for convenient access via `use crate::types::*`.
pub use discovered_files::DiscoveredFiles;
pub use file::DiscoveredFile;
pub use file_references::FileReferences;
pub use symbol::{DiscoveredSymbol, SymbolType};
pub use symbols_collection::DiscoveredSymbols;
