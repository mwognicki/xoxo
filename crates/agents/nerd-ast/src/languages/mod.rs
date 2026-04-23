mod assembly;
mod bash;
mod c;
mod c_sharp;
mod cpp;
mod dart;
mod data;
mod elixir;
mod erlang;
mod fortran;
mod go;
mod graphql;
mod groovy;
mod haskell;
mod java;
mod javascript;
mod julia;
mod kotlin;
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
mod vb_dotnet;
mod zig;

use tree_sitter::Parser;

use crate::{CodeLanguage, CodeStructure, CodeStructureError};

pub(crate) fn inspect_language_structure(
    language: CodeLanguage,
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    match language {
        CodeLanguage::Assembly => assembly::inspect_assembly_structure(content),
        CodeLanguage::Bash => bash::inspect_bash_structure(content),
        CodeLanguage::C => c::inspect_c_structure(content),
        CodeLanguage::CSharp => c_sharp::inspect_c_sharp_structure(content),
        CodeLanguage::Cpp => cpp::inspect_cpp_structure(content),
        CodeLanguage::Dart => dart::inspect_dart_structure(content),
        CodeLanguage::Elixir => elixir::inspect_elixir_structure(content),
        CodeLanguage::Erlang => erlang::inspect_erlang_structure(content),
        CodeLanguage::Fortran => fortran::inspect_fortran_structure(content),
        CodeLanguage::Go => go::inspect_go_structure(content),
        CodeLanguage::Graphql => graphql::inspect_graphql_structure(content),
        CodeLanguage::Groovy => groovy::inspect_groovy_structure(content),
        CodeLanguage::Haskell => haskell::inspect_haskell_structure(content),
        CodeLanguage::Java => java::inspect_java_structure(content),
        CodeLanguage::JavaScript => javascript::inspect_javascript_structure(content),
        CodeLanguage::Julia => julia::inspect_julia_structure(content),
        CodeLanguage::Json => data::inspect_json_structure(content),
        CodeLanguage::Kotlin => kotlin::inspect_kotlin_structure(content),
        CodeLanguage::Lua => lua::inspect_lua_structure(content),
        CodeLanguage::Matlab => matlab::inspect_matlab_structure(content),
        CodeLanguage::Nix => nix::inspect_nix_structure(content),
        CodeLanguage::ObjectiveC => objc::inspect_objc_structure(content),
        CodeLanguage::Pascal => pascal::inspect_pascal_structure(content),
        CodeLanguage::Perl => perl::inspect_perl_structure(content),
        CodeLanguage::TypeScript => javascript::inspect_typescript_structure(content),
        CodeLanguage::Tsx => javascript::inspect_tsx_structure(content),
        CodeLanguage::Php => php::inspect_php_structure(content),
        CodeLanguage::Proto => proto::inspect_proto_structure(content),
        CodeLanguage::Python => python::inspect_python_structure(content),
        CodeLanguage::PowerShell => powershell::inspect_powershell_structure(content),
        CodeLanguage::R => r::inspect_r_structure(content),
        CodeLanguage::Ruby => ruby::inspect_ruby_structure(content),
        CodeLanguage::Rust => rust::inspect_rust_structure(content),
        CodeLanguage::Scala => scala::inspect_scala_structure(content),
        CodeLanguage::Solidity => solidity::inspect_solidity_structure(content),
        CodeLanguage::Swift => swift::inspect_swift_structure(content),
        CodeLanguage::Toml => data::inspect_toml_structure(content),
        CodeLanguage::Yaml => data::inspect_yaml_structure(content),
        CodeLanguage::Zig => zig::inspect_zig_structure(content),
        CodeLanguage::VbDotNet => vb_dotnet::inspect_vb_dotnet_structure(content),
    }
}

pub(crate) fn set_parser_language(
    parser: &mut Parser,
    language: CodeLanguage,
) -> Result<(), CodeStructureError> {
    let result = match language {
        CodeLanguage::Assembly => parser.set_language(&tree_sitter_asm::LANGUAGE.into()),
        CodeLanguage::Bash => parser.set_language(&tree_sitter_bash::LANGUAGE.into()),
        CodeLanguage::C => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
        CodeLanguage::CSharp => parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()),
        CodeLanguage::Cpp => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
        CodeLanguage::Dart => parser.set_language(&tree_sitter_dart::LANGUAGE.into()),
        CodeLanguage::Elixir => parser.set_language(&tree_sitter_elixir::LANGUAGE.into()),
        CodeLanguage::Erlang => parser.set_language(&tree_sitter_erlang::LANGUAGE.into()),
        CodeLanguage::Fortran => parser.set_language(&tree_sitter_fortran::LANGUAGE.into()),
        CodeLanguage::Go => parser.set_language(&tree_sitter_go::LANGUAGE.into()),
        CodeLanguage::Graphql => parser.set_language(&tree_sitter_graphql::LANGUAGE.into()),
        CodeLanguage::Groovy => parser.set_language(&tree_sitter_groovy::LANGUAGE.into()),
        CodeLanguage::Haskell => parser.set_language(&tree_sitter_haskell::LANGUAGE.into()),
        CodeLanguage::Java => parser.set_language(&tree_sitter_java::LANGUAGE.into()),
        CodeLanguage::JavaScript => parser.set_language(&tree_sitter_javascript::LANGUAGE.into()),
        CodeLanguage::Json => parser.set_language(&tree_sitter_json::LANGUAGE.into()),
        CodeLanguage::Julia => parser.set_language(&tree_sitter_julia::LANGUAGE.into()),
        CodeLanguage::Kotlin => parser.set_language(&tree_sitter_kotlin_ng::LANGUAGE.into()),
        CodeLanguage::Lua => parser.set_language(&tree_sitter_lua::LANGUAGE.into()),
        CodeLanguage::Matlab => parser.set_language(&tree_sitter_matlab::LANGUAGE.into()),
        CodeLanguage::Nix => parser.set_language(&tree_sitter_nix::LANGUAGE.into()),
        CodeLanguage::ObjectiveC => parser.set_language(&tree_sitter_objc::LANGUAGE.into()),
        CodeLanguage::Pascal => parser.set_language(&tree_sitter_pascal::LANGUAGE.into()),
        CodeLanguage::Perl => parser.set_language(&tree_sitter_perl::LANGUAGE.into()),
        CodeLanguage::Php => parser.set_language(&tree_sitter_php::LANGUAGE_PHP.into()),
        CodeLanguage::Proto => parser.set_language(&tree_sitter_proto::LANGUAGE.into()),
        CodeLanguage::Python => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
        CodeLanguage::PowerShell => parser.set_language(&tree_sitter_powershell::LANGUAGE.into()),
        CodeLanguage::R => parser.set_language(&tree_sitter_r::LANGUAGE.into()),
        CodeLanguage::Ruby => parser.set_language(&tree_sitter_ruby::LANGUAGE.into()),
        CodeLanguage::Rust => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
        CodeLanguage::Scala => parser.set_language(&tree_sitter_scala::LANGUAGE.into()),
        CodeLanguage::Solidity => parser.set_language(&tree_sitter_solidity::LANGUAGE.into()),
        CodeLanguage::Swift => parser.set_language(&tree_sitter_swift::LANGUAGE.into()),
        CodeLanguage::Toml => parser.set_language(&tree_sitter_toml_ng::LANGUAGE.into()),
        CodeLanguage::TypeScript => {
            parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        }
        CodeLanguage::Tsx => parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()),
        CodeLanguage::VbDotNet => parser.set_language(&tree_sitter_vb_dotnet::LANGUAGE.into()),
        CodeLanguage::Yaml => parser.set_language(&tree_sitter_yaml::LANGUAGE.into()),
        CodeLanguage::Zig => parser.set_language(&tree_sitter_zig::LANGUAGE.into()),
    };

    result.map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))
}
