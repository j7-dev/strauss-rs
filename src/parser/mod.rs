pub mod builtins;
pub mod php_parser;
pub mod reference_extractor;
pub mod symbol_extractor;

pub use builtins::{BUILTIN_CLASSES, BUILTIN_FUNCTIONS, BUILTIN_INTERFACES};
pub use php_parser::PhpParser;
pub use symbol_extractor::{
    extract_symbols, ExtractedClass, ExtractedConstant, ExtractedFunction, ExtractedInterface,
    ExtractedNamespace, ExtractedSymbols, ExtractedTrait,
};
