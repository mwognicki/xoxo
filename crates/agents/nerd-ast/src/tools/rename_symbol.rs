use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::{
    CodeLanguage, CodeStructureError, detect_language, inspect_code_structure, languages,
};

/// Result of an AST-backed symbol rename over in-memory source text.
#[derive(Debug, Clone)]
pub struct RenameSymbolEdit {
    /// Language parser used to compute the rename.
    pub language: CodeLanguage,
    /// Original symbol name.
    pub symbol: String,
    /// Replacement symbol name.
    pub replacement: String,
    /// Number of matching definitions discovered before rewriting.
    pub definition_count: usize,
    /// Number of identifier leaves rewritten.
    pub occurrence_count: usize,
    /// Updated source content.
    pub updated_content: String,
}

/// Errors returned by deterministic symbol rename.
#[derive(Debug)]
pub enum RenameSymbolError {
    /// The replacement is not a conservative identifier.
    InvalidReplacement(String),
    /// The inspected source file contains parse errors.
    SourceHasErrors,
    /// The requested symbol has no AST-backed definition in the file.
    SymbolNotFound(String),
    /// The file extension is not supported by the AST layer.
    UnsupportedLanguage(String),
    /// The parser could not be configured for the detected language.
    ParserConfiguration(String),
    /// The parser did not produce a syntax tree.
    ParseFailed,
}

impl std::fmt::Display for RenameSymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidReplacement(replacement) => {
                write!(f, "invalid replacement symbol: {replacement}")
            }
            Self::SourceHasErrors => write!(f, "source contains parse errors"),
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

impl std::error::Error for RenameSymbolError {}

impl From<CodeStructureError> for RenameSymbolError {
    fn from(error: CodeStructureError) -> Self {
        match error {
            CodeStructureError::UnsupportedLanguage(path) => Self::UnsupportedLanguage(path),
            CodeStructureError::ParserConfiguration(message) => Self::ParserConfiguration(message),
            CodeStructureError::ParseFailed => Self::ParseFailed,
        }
    }
}

/// Rename a symbol in source content using AST-backed identifier ranges.
///
/// # Errors
///
/// Returns [`RenameSymbolError`] when the language is unsupported, parsing
/// fails, the source contains syntax errors, the symbol is not defined in the
/// file, or the replacement is not a conservative identifier.
pub fn rename_symbol_in_content(
    file_path: &Path,
    content: &str,
    symbol: &str,
    replacement: &str,
) -> Result<RenameSymbolEdit, RenameSymbolError> {
    if !is_conservative_identifier(replacement) {
        return Err(RenameSymbolError::InvalidReplacement(
            replacement.to_string(),
        ));
    }

    let structure = inspect_code_structure(file_path, content)?;
    if structure.has_errors {
        return Err(RenameSymbolError::SourceHasErrors);
    }

    let definition_count = structure
        .items
        .iter()
        .filter(|item| item.name.as_deref() == Some(symbol))
        .count();
    if definition_count == 0 {
        return Err(RenameSymbolError::SymbolNotFound(symbol.to_string()));
    }

    let language = detect_language(file_path).ok_or_else(|| {
        RenameSymbolError::UnsupportedLanguage(file_path.display().to_string())
    })?;
    let mut parser = Parser::new();
    languages::set_parser_language(&mut parser, language)?;
    let tree = parser.parse(content, None).ok_or(RenameSymbolError::ParseFailed)?;
    let mut ranges = Vec::new();

    collect_identifier_ranges(
        tree.root_node(),
        content.as_bytes(),
        symbol,
        &mut ranges,
    );
    ranges.sort_unstable();
    ranges.dedup();

    let mut updated_content = content.to_string();
    for (start, end) in ranges.iter().rev() {
        updated_content.replace_range(*start..*end, replacement);
    }

    Ok(RenameSymbolEdit {
        language,
        symbol: symbol.to_string(),
        replacement: replacement.to_string(),
        definition_count,
        occurrence_count: ranges.len(),
        updated_content,
    })
}

fn collect_identifier_ranges(
    node: Node<'_>,
    source: &[u8],
    symbol: &str,
    ranges: &mut Vec<(usize, usize)>,
) {
    if node.child_count() == 0
        && is_identifier_like(node.kind())
        && node.utf8_text(source).ok() == Some(symbol)
    {
        ranges.push((node.start_byte(), node.end_byte()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifier_ranges(child, source, symbol, ranges);
    }
}

fn is_identifier_like(kind: &str) -> bool {
    kind == "identifier"
        || kind == "name"
        || kind == "word"
        || kind.ends_with("_identifier")
        || kind.ends_with("_name")
}

fn is_conservative_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|char| char == '_' || char.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn renames_rust_symbol_identifier_leaves() {
        let content = "struct User;\nfn build(user: User) -> User {\n    User\n}\n// User\n";

        let edit = rename_symbol_in_content(
            Path::new("lib.rs"),
            content,
            "User",
            "Account",
        )
        .unwrap();

        assert_eq!(edit.language, CodeLanguage::Rust);
        assert_eq!(edit.definition_count, 1);
        assert_eq!(edit.occurrence_count, 4);
        assert_eq!(
            edit.updated_content,
            "struct Account;\nfn build(user: Account) -> Account {\n    Account\n}\n// User\n"
        );
    }

    #[test]
    fn rejects_missing_definition() {
        let error = rename_symbol_in_content(
            Path::new("lib.rs"),
            "fn build(value: usize) -> usize { value }\n",
            "User",
            "Account",
        )
        .unwrap_err();

        assert!(matches!(error, RenameSymbolError::SymbolNotFound(_)));
    }

    #[test]
    fn rejects_non_identifier_replacement() {
        let error = rename_symbol_in_content(
            Path::new("lib.rs"),
            "struct User;\n",
            "User",
            "not-valid",
        )
        .unwrap_err();

        assert!(matches!(error, RenameSymbolError::InvalidReplacement(_)));
    }
}
