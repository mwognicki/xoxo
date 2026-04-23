mod language;
mod structs;
mod languages;
pub mod tools;

pub use language::CodeLanguage;
pub use structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure};
pub use tools::{
    EnsureImportEdit, EnsureImportError, FindReferencesOptions, FindSymbolOptions,
    FindTestsForSymbolOptions, PatchSymbolEdit, PatchSymbolError, ReferenceHit,
    ReferenceSearchResult, RenameSymbolEdit, RenameSymbolError, SymbolHit,
    SymbolSearchResult, SymbolTestHit, SymbolTestsResult, ensure_import_in_content,
    find_references, find_symbol, find_tests_for_symbol, patch_symbol_in_content,
    rename_symbol_in_content,
};

use std::path::Path;

use language::detect_language;

/// Errors returned while inspecting source code structure.
#[derive(Debug)]
pub enum CodeStructureError {
    /// The file extension is not mapped to a supported parser.
    UnsupportedLanguage(String),
    /// The parser could not be configured for the detected language.
    ParserConfiguration(String),
    /// The parser did not produce a syntax tree.
    ParseFailed,
}

impl std::fmt::Display for CodeStructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeStructureError::UnsupportedLanguage(path) => {
                write!(f, "unsupported source language for path: {path}")
            }
            CodeStructureError::ParserConfiguration(message) => {
                write!(f, "failed to configure parser: {message}")
            }
            CodeStructureError::ParseFailed => write!(f, "failed to parse source code"),
        }
    }
}

impl std::error::Error for CodeStructureError {}

/// Inspect source code and return deterministic structural facts.
///
/// # Errors
///
/// Returns [`CodeStructureError::UnsupportedLanguage`] when `file_path` cannot
/// be mapped to a supported parser, [`CodeStructureError::ParserConfiguration`]
/// when Tree-sitter rejects the parser language, and
/// [`CodeStructureError::ParseFailed`] when parsing does not produce a tree.
pub fn inspect_code_structure(
    file_path: &Path,
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    let language = detect_language(file_path).ok_or_else(|| {
        CodeStructureError::UnsupportedLanguage(file_path.display().to_string())
    })?;

    languages::inspect_language_structure(language, content)
}
