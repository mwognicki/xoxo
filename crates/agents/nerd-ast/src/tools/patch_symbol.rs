use std::path::Path;

use crate::{
    CodeLanguage, CodeRange, CodeStructureError, detect_language,
    inspect_code_structure,
};

/// Result of an AST-backed symbol definition patch over in-memory source text.
#[derive(Debug, Clone)]
pub struct PatchSymbolEdit {
    /// Language parser used to compute the patch.
    pub language: CodeLanguage,
    /// Symbol whose definition was patched.
    pub symbol: String,
    /// Number of matching definitions discovered before patching.
    pub definition_count: usize,
    /// Range of the patched definition.
    pub range: CodeRange,
    /// Updated source content.
    pub updated_content: String,
}

/// Errors returned by deterministic symbol patching.
#[derive(Debug)]
pub enum PatchSymbolError {
    /// The updated source would contain parse errors.
    ReplacementHasErrors,
    /// The inspected source file contains parse errors.
    SourceHasErrors,
    /// The requested symbol has multiple AST-backed definitions in the file.
    AmbiguousSymbol { symbol: String, definition_count: usize },
    /// The requested symbol has no AST-backed definition in the file.
    SymbolNotFound(String),
    /// The file extension is not supported by the AST layer.
    UnsupportedLanguage(String),
    /// The parser could not be configured for the detected language.
    ParserConfiguration(String),
    /// The parser did not produce a syntax tree.
    ParseFailed,
}

impl std::fmt::Display for PatchSymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReplacementHasErrors => write!(f, "replacement introduces parse errors"),
            Self::SourceHasErrors => write!(f, "source contains parse errors"),
            Self::AmbiguousSymbol {
                symbol,
                definition_count,
            } => write!(
                f,
                "symbol {symbol} has {definition_count} matching definitions"
            ),
            Self::SymbolNotFound(symbol) => write!(f, "symbol not found: {symbol}"),
            Self::UnsupportedLanguage(path) => {
                write!(f, "unsupported source language for path: {path}")
            }
            Self::ParserConfiguration(message) => {
                write!(f, "failed to configure parser: {message}")
            }
            Self::ParseFailed => write!(f, "failed to parse source code"),
        }
    }
}

impl std::error::Error for PatchSymbolError {}

impl From<CodeStructureError> for PatchSymbolError {
    fn from(error: CodeStructureError) -> Self {
        match error {
            CodeStructureError::UnsupportedLanguage(path) => Self::UnsupportedLanguage(path),
            CodeStructureError::ParserConfiguration(message) => Self::ParserConfiguration(message),
            CodeStructureError::ParseFailed => Self::ParseFailed,
        }
    }
}

/// Patch a single symbol definition in source content using its AST range.
///
/// # Errors
///
/// Returns [`PatchSymbolError`] when the language is unsupported, parsing
/// fails, the source contains syntax errors, the symbol is not defined exactly
/// once in the file, or the patched file would contain syntax errors.
pub fn patch_symbol_in_content(
    file_path: &Path,
    content: &str,
    symbol: &str,
    replacement: &str,
) -> Result<PatchSymbolEdit, PatchSymbolError> {
    let structure = inspect_code_structure(file_path, content)?;
    if structure.has_errors {
        return Err(PatchSymbolError::SourceHasErrors);
    }

    let matches = structure
        .items
        .iter()
        .filter(|item| item.name.as_deref() == Some(symbol))
        .collect::<Vec<_>>();
    let definition_count = matches.len();

    let Some(item) = matches.first() else {
        return Err(PatchSymbolError::SymbolNotFound(symbol.to_string()));
    };
    if definition_count > 1 {
        return Err(PatchSymbolError::AmbiguousSymbol {
            symbol: symbol.to_string(),
            definition_count,
        });
    }

    let range = item.range;
    let mut updated_content = content.to_string();
    updated_content.replace_range(range.start_byte..range.end_byte, replacement);

    let updated_structure = inspect_code_structure(file_path, &updated_content)?;
    if updated_structure.has_errors {
        return Err(PatchSymbolError::ReplacementHasErrors);
    }

    let language = detect_language(file_path).ok_or_else(|| {
        PatchSymbolError::UnsupportedLanguage(file_path.display().to_string())
    })?;

    Ok(PatchSymbolEdit {
        language,
        symbol: symbol.to_string(),
        definition_count,
        range,
        updated_content,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn patches_single_rust_symbol_definition() {
        let content = "fn boot() -> i32 { 1 }\nfn keep() -> i32 { 3 }\n";

        let edit = patch_symbol_in_content(
            Path::new("lib.rs"),
            content,
            "boot",
            "fn boot() -> i32 { 2 }",
        )
        .unwrap();

        assert_eq!(edit.language, CodeLanguage::Rust);
        assert_eq!(edit.definition_count, 1);
        assert_eq!(edit.range.start_line, 1);
        assert_eq!(
            edit.updated_content,
            "fn boot() -> i32 { 2 }\nfn keep() -> i32 { 3 }\n"
        );
    }

    #[test]
    fn rejects_ambiguous_symbol_definitions() {
        let error = patch_symbol_in_content(
            Path::new("lib.rs"),
            "fn boot() {}\nfn boot() {}\n",
            "boot",
            "fn boot() {}\n",
        )
        .unwrap_err();

        assert!(matches!(error, PatchSymbolError::AmbiguousSymbol { .. }));
    }

    #[test]
    fn rejects_replacement_with_parse_errors() {
        let error = patch_symbol_in_content(
            Path::new("lib.rs"),
            "fn boot() {}\n",
            "boot",
            "fn boot(",
        )
        .unwrap_err();

        assert!(matches!(error, PatchSymbolError::ReplacementHasErrors));
    }
}
