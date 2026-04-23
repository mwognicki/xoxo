use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Serialize;
use tree_sitter::{Node, Parser};

use crate::{
    CodeLanguage, CodeRange, CodeStructureError, detect_language, languages,
};

/// Options for deterministic AST-backed reference search.
#[derive(Debug, Clone)]
pub struct FindReferencesOptions {
    /// Exact symbol name to find references for.
    pub symbol: String,
    /// Optional file or directory scope. Defaults to the current working directory.
    pub scope: Option<PathBuf>,
}

/// Result of an AST-backed reference search.
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceSearchResult {
    /// Exact symbol name searched for.
    pub symbol: String,
    /// File or directory scope searched.
    pub scope: String,
    /// Number of matching references.
    pub hit_count: usize,
    /// Matching references in deterministic file and source order.
    pub hits: Vec<ReferenceHit>,
}

/// A single identifier-like AST leaf matching the requested symbol.
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceHit {
    /// File path relative to the searched scope when possible.
    pub file_path: String,
    /// Language parser that found the reference.
    pub language: CodeLanguage,
    /// One-indexed line containing the reference.
    pub line: usize,
    /// Source line containing the reference.
    pub line_text: String,
    /// Exact AST leaf range.
    pub range: CodeRange,
}

/// Find exact symbol references under a file or directory using AST parsers.
///
/// # Errors
///
/// Returns [`CodeStructureError::ParserConfiguration`] when the current
/// directory cannot be determined or when Tree-sitter rejects a parser
/// language. Files that cannot be read or are unsupported are skipped.
pub fn find_references(
    options: FindReferencesOptions,
) -> Result<ReferenceSearchResult, CodeStructureError> {
    let scope = match options.scope {
        Some(scope) => scope,
        None => std::env::current_dir()
            .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?,
    };

    let mut hits = Vec::new();
    if scope.is_file() {
        collect_file_references(&scope, &scope, &options.symbol, &mut hits)?;
    } else {
        for result in WalkBuilder::new(&scope).standard_filters(true).build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if !entry
                .file_type()
                .map(|file_type| file_type.is_file())
                .unwrap_or(false)
            {
                continue;
            }

            collect_file_references(entry.path(), &scope, &options.symbol, &mut hits)?;
        }
    }

    hits.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.range.start_line.cmp(&right.range.start_line))
            .then_with(|| left.range.start_byte.cmp(&right.range.start_byte))
    });

    Ok(ReferenceSearchResult {
        symbol: options.symbol,
        scope: scope.to_string_lossy().into_owned(),
        hit_count: hits.len(),
        hits,
    })
}

fn collect_file_references(
    path: &Path,
    scope: &Path,
    symbol: &str,
    hits: &mut Vec<ReferenceHit>,
) -> Result<(), CodeStructureError> {
    let Some(language) = detect_language(path) else {
        return Ok(());
    };
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Ok(()),
    };

    let mut parser = Parser::new();
    languages::set_parser_language(&mut parser, language)?;
    let tree = parser
        .parse(&content, None)
        .ok_or(CodeStructureError::ParseFailed)?;

    let mut ranges = Vec::new();
    collect_identifier_ranges(
        tree.root_node(),
        content.as_bytes(),
        symbol,
        &mut ranges,
    );

    for range in ranges {
        hits.push(ReferenceHit {
            file_path: relative_to(scope, path),
            language,
            line: range.start_line,
            line_text: line_text(&content, range.start_line),
            range,
        });
    }

    Ok(())
}

fn collect_identifier_ranges(
    node: Node<'_>,
    source: &[u8],
    symbol: &str,
    ranges: &mut Vec<CodeRange>,
) {
    if node.child_count() == 0
        && is_identifier_like(node.kind())
        && node.utf8_text(source).ok() == Some(symbol)
    {
        ranges.push(CodeRange {
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        });
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

fn line_text(content: &str, line: usize) -> String {
    content
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or_default()
        .to_string()
}

fn relative_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_identifier_references_without_comments_or_strings() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("lib.rs"),
            "struct User;\nfn build(user: User) -> User {\n    let label = \"User\";\n    User\n}\n// User\n",
        )
        .unwrap();

        let result = find_references(FindReferencesOptions {
            symbol: "User".to_string(),
            scope: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let lines = result
            .hits
            .iter()
            .map(|hit| hit.line)
            .collect::<Vec<_>>();

        assert_eq!(result.hit_count, 4);
        assert_eq!(lines, vec![1, 2, 2, 4]);
    }

    #[test]
    fn searches_single_file_scope() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("main.py");
        fs::write(
            &file_path,
            "class User:\n    pass\n\ndef build() -> User:\n    return User()\n",
        )
        .unwrap();

        let result = find_references(FindReferencesOptions {
            symbol: "User".to_string(),
            scope: Some(file_path),
        })
        .unwrap();

        assert_eq!(result.hit_count, 3);
        assert!(result.hits.iter().all(|hit| hit.language == CodeLanguage::Python));
    }
}
