//! AST-backed source-code inspection.

mod bash;
mod c;
mod c_sharp;
mod cpp;
mod data;
mod find_symbol;
mod go;
mod javascript;
mod language;
mod lua;
mod perl;
mod php;
mod python;
mod ruby;
mod rust;
mod swift;
mod structs;

pub use find_symbol::{FindSymbolOptions, SymbolHit, SymbolSearchResult, find_symbol};
pub use language::CodeLanguage;
pub use structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure};

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
    match detect_language(file_path) {
        Some(CodeLanguage::Bash) => bash::inspect_bash_structure(content),
        Some(CodeLanguage::C) => c::inspect_c_structure(content),
        Some(CodeLanguage::CSharp) => c_sharp::inspect_c_sharp_structure(content),
        Some(CodeLanguage::Cpp) => cpp::inspect_cpp_structure(content),
        Some(CodeLanguage::Go) => go::inspect_go_structure(content),
        Some(CodeLanguage::JavaScript) => javascript::inspect_javascript_structure(content),
        Some(CodeLanguage::Json) => data::inspect_json_structure(content),
        Some(CodeLanguage::Lua) => lua::inspect_lua_structure(content),
        Some(CodeLanguage::Perl) => perl::inspect_perl_structure(content),
        Some(CodeLanguage::TypeScript) => javascript::inspect_typescript_structure(content),
        Some(CodeLanguage::Tsx) => javascript::inspect_tsx_structure(content),
        Some(CodeLanguage::Php) => php::inspect_php_structure(content),
        Some(CodeLanguage::Python) => python::inspect_python_structure(content),
        Some(CodeLanguage::Ruby) => ruby::inspect_ruby_structure(content),
        Some(CodeLanguage::Rust) => rust::inspect_rust_structure(content),
        Some(CodeLanguage::Swift) => swift::inspect_swift_structure(content),
        Some(CodeLanguage::Toml) => data::inspect_toml_structure(content),
        Some(CodeLanguage::Yaml) => data::inspect_yaml_structure(content),
        None => Err(CodeStructureError::UnsupportedLanguage(
            file_path.display().to_string(),
        )),
    }
}
