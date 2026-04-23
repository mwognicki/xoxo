use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Serialize;
use tree_sitter::{Node, Parser};

use crate::{
    CodeItemKind, CodeLanguage, CodeRange, CodeStructureError, detect_language,
    inspect_code_structure, languages,
};

/// Options for deterministic AST-backed test discovery.
#[derive(Debug, Clone)]
pub struct FindTestsForSymbolOptions {
    /// Source file containing or using the target symbol.
    pub file_path: PathBuf,
    /// Exact symbol name to connect to tests.
    pub symbol: String,
}

/// Result of an AST-backed test discovery search.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolTestsResult {
    /// Source file passed by the caller.
    pub file_path: String,
    /// Exact symbol name searched for.
    pub symbol: String,
    /// Root searched for related tests.
    pub search_root: String,
    /// Number of matching tests.
    pub hit_count: usize,
    /// Matching tests in deterministic file and source order.
    pub tests: Vec<SymbolTestHit>,
}

/// A test-like function or method that references the requested symbol.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolTestHit {
    /// File path relative to the searched root when possible.
    pub file_path: String,
    /// Language parser that found the test.
    pub language: CodeLanguage,
    /// Test function or method name.
    pub name: String,
    /// Test item kind.
    pub kind: CodeItemKind,
    /// Source range covered by the test item.
    pub range: CodeRange,
    /// One-indexed lines where the symbol is referenced inside the test item.
    pub reference_lines: Vec<usize>,
}

/// Find test-like functions or methods that reference a symbol.
///
/// # Errors
///
/// Returns [`CodeStructureError`] when the source language is unsupported, the
/// current directory cannot be determined, or a parser fails to configure.
/// Files that cannot be read or are unsupported are skipped.
pub fn find_tests_for_symbol(
    options: FindTestsForSymbolOptions,
) -> Result<SymbolTestsResult, CodeStructureError> {
    let source_language = detect_language(&options.file_path).ok_or_else(|| {
        CodeStructureError::UnsupportedLanguage(options.file_path.display().to_string())
    })?;
    let search_root = search_root_for(&options.file_path)?;
    let mut tests = Vec::new();

    for result in WalkBuilder::new(&search_root).standard_filters(true).build() {
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

        let path = entry.path();
        if detect_language(path) != Some(source_language) {
            continue;
        }
        if path != options.file_path && !is_test_path(path) {
            continue;
        }

        collect_file_tests(
            path,
            &search_root,
            source_language,
            &options.symbol,
            &mut tests,
        )?;
    }

    tests.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.range.start_line.cmp(&right.range.start_line))
            .then_with(|| left.range.start_byte.cmp(&right.range.start_byte))
    });

    Ok(SymbolTestsResult {
        file_path: options.file_path.to_string_lossy().into_owned(),
        symbol: options.symbol,
        search_root: search_root.to_string_lossy().into_owned(),
        hit_count: tests.len(),
        tests,
    })
}

fn collect_file_tests(
    path: &Path,
    search_root: &Path,
    language: CodeLanguage,
    symbol: &str,
    tests: &mut Vec<SymbolTestHit>,
) -> Result<(), CodeStructureError> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Ok(()),
    };
    let structure = match inspect_code_structure(path, &content) {
        Ok(structure) => structure,
        Err(CodeStructureError::UnsupportedLanguage(_)) => return Ok(()),
        Err(error) => return Err(error),
    };
    if structure.has_errors {
        return Ok(());
    }

    let references = identifier_reference_ranges(path, language, &content, symbol)?;
    for item in structure.items {
        if !matches!(item.kind, CodeItemKind::Function | CodeItemKind::Method) {
            continue;
        }
        let name = match item.name {
            Some(name) => name,
            None => continue,
        };
        if !is_test_like(path, &name, &content, item.range) {
            continue;
        }

        let reference_lines = references
            .iter()
            .filter(|range| range_within(**range, item.range))
            .map(|range| range.start_line)
            .collect::<Vec<_>>();
        if reference_lines.is_empty() {
            continue;
        }

        tests.push(SymbolTestHit {
            file_path: relative_to(search_root, path),
            language,
            name,
            kind: item.kind,
            range: item.range,
            reference_lines,
        });
    }

    Ok(())
}

fn identifier_reference_ranges(
    path: &Path,
    language: CodeLanguage,
    content: &str,
    symbol: &str,
) -> Result<Vec<CodeRange>, CodeStructureError> {
    let mut parser = Parser::new();
    languages::set_parser_language(&mut parser, language)?;
    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let mut ranges = Vec::new();
    collect_identifier_ranges(tree.root_node(), content.as_bytes(), symbol, &mut ranges);

    if ranges.is_empty() && detect_language(path) == Some(language) {
        return Ok(Vec::new());
    }

    Ok(ranges)
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

fn is_test_like(path: &Path, name: &str, content: &str, range: CodeRange) -> bool {
    is_test_path(path) || is_test_name(name) || has_test_attribute(content, range.start_line)
}

fn is_test_path(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        matches!(value.as_str(), "test" | "tests" | "__tests__")
    }) || path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            let stem = stem.to_ascii_lowercase();
            stem.starts_with("test_")
                || stem.ends_with("_test")
                || stem.ends_with("_tests")
                || stem.ends_with("_spec")
        })
        .unwrap_or(false)
}

fn is_test_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("test") || name.contains("spec") || name.starts_with("should_")
}

fn has_test_attribute(content: &str, start_line: usize) -> bool {
    let lines = content.lines().collect::<Vec<_>>();
    let start_index = start_line.saturating_sub(1);
    let from = start_index.saturating_sub(3);

    lines
        .get(from..start_index)
        .unwrap_or_default()
        .iter()
        .any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("#[") && trimmed.contains("test")
        })
}

fn is_identifier_like(kind: &str) -> bool {
    kind == "identifier"
        || kind == "name"
        || kind == "word"
        || kind.ends_with("_identifier")
        || kind.ends_with("_name")
}

fn range_within(inner: CodeRange, outer: CodeRange) -> bool {
    inner.start_byte >= outer.start_byte && inner.end_byte <= outer.end_byte
}

fn search_root_for(file_path: &Path) -> Result<PathBuf, CodeStructureError> {
    let absolute = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?
            .join(file_path)
    };

    let mut current = absolute.parent();
    while let Some(dir) = current {
        if is_project_root(dir) {
            return Ok(dir.to_path_buf());
        }
        current = dir.parent();
    }

    Ok(absolute
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(".")))
}

fn is_project_root(path: &Path) -> bool {
    ["Cargo.toml", "package.json", "pyproject.toml", "go.mod", ".git"]
        .iter()
        .any(|marker| path.join(marker).exists())
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
    fn finds_rust_unit_tests_for_symbol() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("lib.rs");
        fs::write(
            &file_path,
            "struct User;\n\n#[test]\nfn builds_user() {\n    let _user = User;\n}\n\nfn helper() { let _user = User; }\n",
        )
        .unwrap();

        let result = find_tests_for_symbol(FindTestsForSymbolOptions {
            file_path,
            symbol: "User".to_string(),
        })
        .unwrap();

        assert_eq!(result.hit_count, 1);
        assert_eq!(result.tests[0].name, "builds_user");
        assert_eq!(result.tests[0].reference_lines, vec![5]);
    }

    #[test]
    fn finds_integration_test_files_for_symbol() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        let src = temp.path().join("src");
        let tests = temp.path().join("tests");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&tests).unwrap();
        let file_path = src.join("lib.rs");
        fs::write(&file_path, "pub struct User;\n").unwrap();
        fs::write(
            tests.join("user_test.rs"),
            "use demo::User;\n\nfn creates_user() {\n    let _user = User;\n}\n",
        )
        .unwrap();

        let result = find_tests_for_symbol(FindTestsForSymbolOptions {
            file_path,
            symbol: "User".to_string(),
        })
        .unwrap();

        assert_eq!(result.hit_count, 1);
        assert_eq!(result.tests[0].file_path, "tests/user_test.rs");
    }
}
