//! AST-backed source-code inspection.

mod assembly;
mod bash;
mod c;
mod c_sharp;
mod cpp;
mod data;
mod dart;
mod elixir;
mod erlang;
mod find_symbol;
mod fortran;
mod go;
mod graphql;
mod groovy;
mod haskell;
mod java;
mod javascript;
mod julia;
mod kotlin;
mod language;
mod lua;
mod matlab;
mod nix;
mod objc;
mod pascal;
mod perl;
mod php;
mod powershell;
mod proto;
mod python;
mod r;
mod ruby;
mod rust;
mod scala;
mod solidity;
mod swift;
mod structs;
mod zig;
mod vb_dotnet;

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
        Some(CodeLanguage::Assembly) => assembly::inspect_assembly_structure(content),
        Some(CodeLanguage::Bash) => bash::inspect_bash_structure(content),
        Some(CodeLanguage::C) => c::inspect_c_structure(content),
        Some(CodeLanguage::CSharp) => c_sharp::inspect_c_sharp_structure(content),
        Some(CodeLanguage::Cpp) => cpp::inspect_cpp_structure(content),
        Some(CodeLanguage::Dart) => dart::inspect_dart_structure(content),
        Some(CodeLanguage::Elixir) => elixir::inspect_elixir_structure(content),
        Some(CodeLanguage::Erlang) => erlang::inspect_erlang_structure(content),
        Some(CodeLanguage::Fortran) => fortran::inspect_fortran_structure(content),
        Some(CodeLanguage::Go) => go::inspect_go_structure(content),
        Some(CodeLanguage::Graphql) => graphql::inspect_graphql_structure(content),
        Some(CodeLanguage::Groovy) => groovy::inspect_groovy_structure(content),
        Some(CodeLanguage::Haskell) => haskell::inspect_haskell_structure(content),
        Some(CodeLanguage::Java) => java::inspect_java_structure(content),
        Some(CodeLanguage::JavaScript) => javascript::inspect_javascript_structure(content),
        Some(CodeLanguage::Julia) => julia::inspect_julia_structure(content),
        Some(CodeLanguage::Json) => data::inspect_json_structure(content),
        Some(CodeLanguage::Kotlin) => kotlin::inspect_kotlin_structure(content),
        Some(CodeLanguage::Lua) => lua::inspect_lua_structure(content),
        Some(CodeLanguage::Matlab) => matlab::inspect_matlab_structure(content),
        Some(CodeLanguage::Nix) => nix::inspect_nix_structure(content),
        Some(CodeLanguage::ObjectiveC) => objc::inspect_objc_structure(content),
        Some(CodeLanguage::Pascal) => pascal::inspect_pascal_structure(content),
        Some(CodeLanguage::Perl) => perl::inspect_perl_structure(content),
        Some(CodeLanguage::TypeScript) => javascript::inspect_typescript_structure(content),
        Some(CodeLanguage::Tsx) => javascript::inspect_tsx_structure(content),
        Some(CodeLanguage::Php) => php::inspect_php_structure(content),
        Some(CodeLanguage::Proto) => proto::inspect_proto_structure(content),
        Some(CodeLanguage::Python) => python::inspect_python_structure(content),
        Some(CodeLanguage::PowerShell) => powershell::inspect_powershell_structure(content),
        Some(CodeLanguage::R) => r::inspect_r_structure(content),
        Some(CodeLanguage::Ruby) => ruby::inspect_ruby_structure(content),
        Some(CodeLanguage::Rust) => rust::inspect_rust_structure(content),
        Some(CodeLanguage::Scala) => scala::inspect_scala_structure(content),
        Some(CodeLanguage::Solidity) => solidity::inspect_solidity_structure(content),
        Some(CodeLanguage::Swift) => swift::inspect_swift_structure(content),
        Some(CodeLanguage::Toml) => data::inspect_toml_structure(content),
        Some(CodeLanguage::Yaml) => data::inspect_yaml_structure(content),
        Some(CodeLanguage::Zig) => zig::inspect_zig_structure(content),
        Some(CodeLanguage::VbDotNet) => vb_dotnet::inspect_vb_dotnet_structure(content),
        None => Err(CodeStructureError::UnsupportedLanguage(
            file_path.display().to_string(),
        )),
    }
}
