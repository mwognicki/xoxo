use std::path::Path;

use crate::{
    CodeItemKind, CodeLanguage, CodeRange, CodeStructureError, detect_language,
    inspect_code_structure,
};

/// Result of an AST-backed import insertion over in-memory source text.
#[derive(Debug, Clone)]
pub struct EnsureImportEdit {
    /// Language parser used to compute the import placement.
    pub language: CodeLanguage,
    /// Import source requested by the caller.
    pub import_spec: String,
    /// Whether the source content changed.
    pub changed: bool,
    /// Byte where the import was inserted when changed.
    pub inserted_at_byte: Option<usize>,
    /// Updated source content.
    pub updated_content: String,
}

/// Errors returned by deterministic import insertion.
#[derive(Debug)]
pub enum EnsureImportError {
    /// The import specification is empty after trimming whitespace.
    EmptyImportSpec,
    /// The inspected source file contains parse errors.
    SourceHasErrors,
    /// The updated source would contain parse errors.
    ImportIntroducesErrors,
    /// The file extension is not supported by the AST layer.
    UnsupportedLanguage(String),
    /// The parser could not be configured for the detected language.
    ParserConfiguration(String),
    /// The parser did not produce a syntax tree.
    ParseFailed,
}

impl std::fmt::Display for EnsureImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyImportSpec => write!(f, "import_spec cannot be empty"),
            Self::SourceHasErrors => write!(f, "source contains parse errors"),
            Self::ImportIntroducesErrors => write!(f, "import_spec introduces parse errors"),
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

impl std::error::Error for EnsureImportError {}

impl From<CodeStructureError> for EnsureImportError {
    fn from(error: CodeStructureError) -> Self {
        match error {
            CodeStructureError::UnsupportedLanguage(path) => Self::UnsupportedLanguage(path),
            CodeStructureError::ParserConfiguration(message) => Self::ParserConfiguration(message),
            CodeStructureError::ParseFailed => Self::ParseFailed,
        }
    }
}

/// Ensure an import exists in source content using AST-backed import placement.
///
/// # Errors
///
/// Returns [`EnsureImportError`] when the language is unsupported, parsing
/// fails, the original source contains syntax errors, the import spec is empty,
/// or the updated file would contain syntax errors.
pub fn ensure_import_in_content(
    file_path: &Path,
    content: &str,
    import_spec: &str,
) -> Result<EnsureImportEdit, EnsureImportError> {
    let import_spec = normalize_import_spec(import_spec)?;
    let structure = inspect_code_structure(file_path, content)?;
    if structure.has_errors {
        return Err(EnsureImportError::SourceHasErrors);
    }

    let language = detect_language(file_path).ok_or_else(|| {
        EnsureImportError::UnsupportedLanguage(file_path.display().to_string())
    })?;
    let import_ranges = structure
        .items
        .iter()
        .filter(|item| item.kind == CodeItemKind::Import)
        .map(|item| item.range)
        .collect::<Vec<_>>();

    if import_already_present(content, &import_spec, &import_ranges) {
        return Ok(EnsureImportEdit {
            language,
            import_spec,
            changed: false,
            inserted_at_byte: None,
            updated_content: content.to_string(),
        });
    }

    let ending = detect_line_ending(content);
    let rendered_import = import_spec.replace('\n', ending);
    let insertion_byte = insertion_byte(content, language, &import_ranges);
    let updated_content = insert_import(content, insertion_byte, &rendered_import, ending);

    let updated_structure = inspect_code_structure(file_path, &updated_content)?;
    if updated_structure.has_errors {
        return Err(EnsureImportError::ImportIntroducesErrors);
    }

    Ok(EnsureImportEdit {
        language,
        import_spec,
        changed: true,
        inserted_at_byte: Some(insertion_byte),
        updated_content,
    })
}

fn normalize_import_spec(import_spec: &str) -> Result<String, EnsureImportError> {
    let normalized = import_spec.trim();
    if normalized.is_empty() {
        return Err(EnsureImportError::EmptyImportSpec);
    }

    Ok(normalized.replace("\r\n", "\n").replace('\r', "\n"))
}

fn import_already_present(
    content: &str,
    import_spec: &str,
    import_ranges: &[CodeRange],
) -> bool {
    import_ranges.iter().any(|range| {
        content
            .get(range.start_byte..range.end_byte)
            .map(normalized_block)
            .as_deref()
            == Some(import_spec)
    }) || content
        .lines()
        .any(|line| normalized_block(line) == import_spec)
}

fn normalized_block(value: &str) -> String {
    value.trim().replace("\r\n", "\n").replace('\r', "\n")
}

fn insertion_byte(
    content: &str,
    language: CodeLanguage,
    import_ranges: &[CodeRange],
) -> usize {
    if let Some(last_import) = import_ranges.iter().max_by_key(|range| range.end_byte) {
        return last_import.end_byte;
    }

    match language {
        CodeLanguage::Go => first_line_end(content).filter(|_| content.starts_with("package ")),
        CodeLanguage::Python | CodeLanguage::Bash => shebang_line_end(content),
        _ => None,
    }
    .unwrap_or(0)
}

fn insert_import(
    content: &str,
    insertion_byte: usize,
    import_spec: &str,
    ending: &str,
) -> String {
    if content.is_empty() {
        return format!("{import_spec}{ending}");
    }

    if insertion_byte == 0 {
        return format!("{import_spec}{ending}{content}");
    }

    format!(
        "{}{ending}{import_spec}{}",
        &content[..insertion_byte],
        &content[insertion_byte..],
    )
}

fn first_line_end(content: &str) -> Option<usize> {
    content.find('\n').map(|index| index + 1)
}

fn shebang_line_end(content: &str) -> Option<usize> {
    content
        .starts_with("#!")
        .then(|| first_line_end(content))
        .flatten()
}

fn detect_line_ending(content: &str) -> &str {
    if content.contains("\r\n") {
        "\r\n"
    } else if content.contains('\n') {
        "\n"
    } else if content.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn appends_rust_use_after_existing_imports() {
        let edit = ensure_import_in_content(
            Path::new("lib.rs"),
            "use std::fs;\n\nfn main() {}\n",
            "use std::path::Path;",
        )
        .unwrap();

        assert!(edit.changed);
        assert_eq!(
            edit.updated_content,
            "use std::fs;\nuse std::path::Path;\n\nfn main() {}\n"
        );
    }

    #[test]
    fn leaves_existing_import_unchanged() {
        let edit = ensure_import_in_content(
            Path::new("lib.rs"),
            "use std::fs;\n\nfn main() {}\n",
            "use std::fs;",
        )
        .unwrap();

        assert!(!edit.changed);
        assert_eq!(edit.updated_content, "use std::fs;\n\nfn main() {}\n");
    }

    #[test]
    fn inserts_go_import_after_package_line() {
        let edit = ensure_import_in_content(
            Path::new("main.go"),
            "package main\n\nfunc main() {}\n",
            "import \"fmt\"",
        )
        .unwrap();

        assert_eq!(
            edit.updated_content,
            "package main\n\nimport \"fmt\"\nfunc main() {}\n"
        );
    }

    #[test]
    fn rejects_imports_that_do_not_parse() {
        let error = ensure_import_in_content(
            Path::new("lib.rs"),
            "fn main() {}\n",
            "use ;",
        )
        .unwrap_err();

        assert!(matches!(error, EnsureImportError::ImportIntroducesErrors));
    }
}
