//! Noise stripper module.
//!
//! Provides functionality to strip noise from supported source code files.

use std::path::Path;

mod c_family;
mod c_sharp;
mod dart;
mod elixir;
mod erlang;
mod fortran;
mod go;
mod graphql;
mod groovy;
mod haskell;
mod javascript;
mod julia;
mod kotlin;
mod lua;
mod php;
mod python;
mod rust;
mod ruby;
mod scala;
mod shell;
mod swift;
mod toml;
mod vb_net;
mod yaml;

/// Source languages currently supported by the noise stripper pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Jsx,
    Tsx,
    Vue,
    Python,
    Php,
    CFamily,
    CSharp,
    Ruby,
    Shell,
    GraphQl,
    Java,
    Solidity,
    Kotlin,
    Swift,
    Elixir,
    Groovy,
    Dart,
    Haskell,
    Erlang,
    Fortran,
    Julia,
    Lua,
    Scala,
    VbNet,
    Go,
    Toml,
    Yaml,
}

trait LanguageDetector {
    fn detect_language(&self, file_path: Option<&Path>, content: &str) -> Option<SourceLanguage>;
}

#[cfg(not(feature = "extension-language-detection"))]
struct NoopLanguageDetector;

#[cfg(not(feature = "extension-language-detection"))]
impl LanguageDetector for NoopLanguageDetector {
    fn detect_language(&self, _file_path: Option<&Path>, _content: &str) -> Option<SourceLanguage> {
        None
    }
}

struct ExtensionLanguageDetector;

// @todo Probably we can use the unified language detector, since it's shared with nerd-ast
impl LanguageDetector for ExtensionLanguageDetector {
    fn detect_language(&self, file_path: Option<&Path>, _content: &str) -> Option<SourceLanguage> {
        let extension = file_path
            .and_then(Path::extension)
            .and_then(|ext| ext.to_str())?;

        match extension {
            "rs" => Some(SourceLanguage::Rust),
            "ts" => Some(SourceLanguage::TypeScript),
            "js" => Some(SourceLanguage::JavaScript),
            "tsx" => Some(SourceLanguage::Tsx),
            "jsx" => Some(SourceLanguage::Jsx),
            "vue" => Some(SourceLanguage::Vue),
            "py" => Some(SourceLanguage::Python),
            "php" => Some(SourceLanguage::Php),
            "cs" => Some(SourceLanguage::CSharp),
            "rb" => Some(SourceLanguage::Ruby),
            "sh" | "bash" | "zsh" | "ksh" => Some(SourceLanguage::Shell),
            "graphql" | "gql" => Some(SourceLanguage::GraphQl),
            "java" => Some(SourceLanguage::Java),
            "sol" => Some(SourceLanguage::Solidity),
            "kt" | "kts" => Some(SourceLanguage::Kotlin),
            "swift" => Some(SourceLanguage::Swift),
            "ex" | "exs" => Some(SourceLanguage::Elixir),
            "groovy" | "gradle" => Some(SourceLanguage::Groovy),
            "dart" => Some(SourceLanguage::Dart),
            "hs" => Some(SourceLanguage::Haskell),
            "erl" | "hrl" => Some(SourceLanguage::Erlang),
            "f" | "for" | "f90" | "f95" | "f03" | "f08" => Some(SourceLanguage::Fortran),
            "jl" => Some(SourceLanguage::Julia),
            "lua" => Some(SourceLanguage::Lua),
            "scala" | "sc" => Some(SourceLanguage::Scala),
            "vb" => Some(SourceLanguage::VbNet),
            "c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => {
                Some(SourceLanguage::CFamily)
            }
            "go" => Some(SourceLanguage::Go),
            "toml" => Some(SourceLanguage::Toml),
            "yaml" | "yml" => Some(SourceLanguage::Yaml),
            _ => None,
        }
    }
}

#[cfg(feature = "extension-language-detection")]
fn default_language_detector() -> &'static dyn LanguageDetector {
    &ExtensionLanguageDetector
}

#[cfg(not(feature = "extension-language-detection"))]
fn default_language_detector() -> &'static dyn LanguageDetector {
    &NoopLanguageDetector
}

fn detect_source_language(file_path: Option<&Path>, content: &str) -> Option<SourceLanguage> {
    default_language_detector().detect_language(file_path, content)
}

fn strip_noise_for_language(language: SourceLanguage, content: &str) -> String {
    match language {
        SourceLanguage::Rust => rust::strip_rust_noise(content),
        SourceLanguage::TypeScript | SourceLanguage::JavaScript => {
            javascript::strip_javascript_noise(content, javascript::JavaScriptFlavor::Standard)
        }
        SourceLanguage::Jsx | SourceLanguage::Tsx => {
            javascript::strip_javascript_noise(content, javascript::JavaScriptFlavor::Jsx)
        }
        SourceLanguage::Vue => {
            javascript::strip_javascript_noise(content, javascript::JavaScriptFlavor::Vue)
        }
        SourceLanguage::Python => python::strip_python_noise(content),
        SourceLanguage::Php => php::strip_php_noise(content),
        SourceLanguage::CFamily => c_family::strip_c_family_noise(content),
        SourceLanguage::CSharp => c_sharp::strip_c_sharp_noise(content),
        SourceLanguage::Ruby => ruby::strip_ruby_noise(content),
        SourceLanguage::Shell => shell::strip_shell_noise(content),
        SourceLanguage::GraphQl => graphql::strip_graphql_noise(content),
        SourceLanguage::Java | SourceLanguage::Solidity => c_family::strip_c_family_noise(content),
        SourceLanguage::Kotlin => kotlin::strip_kotlin_noise(content),
        SourceLanguage::Swift => swift::strip_swift_noise(content),
        SourceLanguage::Elixir => elixir::strip_elixir_noise(content),
        SourceLanguage::Groovy => groovy::strip_groovy_noise(content),
        SourceLanguage::Dart => dart::strip_dart_noise(content),
        SourceLanguage::Haskell => haskell::strip_haskell_noise(content),
        SourceLanguage::Erlang => erlang::strip_erlang_noise(content),
        SourceLanguage::Fortran => fortran::strip_fortran_noise(content),
        SourceLanguage::Julia => julia::strip_julia_noise(content),
        SourceLanguage::Lua => lua::strip_lua_noise(content),
        SourceLanguage::Scala => scala::strip_scala_noise(content),
        SourceLanguage::VbNet => vb_net::strip_vb_net_noise(content),
        SourceLanguage::Go => go::strip_go_noise(content),
        SourceLanguage::Toml => toml::strip_toml_noise(content),
        SourceLanguage::Yaml => yaml::strip_yaml_noise(content),
    }
}

/// Strip noise from supported source code content.
///
/// # Arguments
///
/// * `file_path` - Optional source file path used for language detection
/// * `content` - The text content to process
///
/// # Returns
///
/// * `String` - The processed content with noise removed when supported
///
/// # Notes
///
/// Unsupported or undetected file types are returned unchanged.
/// Comment stripping is not implemented yet; supported languages are also
/// returned unchanged for now.
pub fn strip_noise(file_path: Option<&Path>, content: &str) -> String {
    let Some(language) = detect_source_language(file_path, content) else {
        return content.to_string();
    };

    strip_noise_for_language(language, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_noise_unchanged_for_plain_text() {
        let input = "Some test content";
        let result = strip_noise(None, input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_noise_empty() {
        let input = "";
        let result = strip_noise(None, input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_noise_multiline() {
        let input = "Line 1\nLine 2\nLine 3";
        let result = strip_noise(None, input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extension_detector_detects_supported_languages() {
        let detector = ExtensionLanguageDetector;

        assert_eq!(
            detector.detect_language(Some(Path::new("main.rs")), ""),
            Some(SourceLanguage::Rust)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("index.ts")), ""),
            Some(SourceLanguage::TypeScript)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("index.js")), ""),
            Some(SourceLanguage::JavaScript)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("view.jsx")), ""),
            Some(SourceLanguage::Jsx)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("view.tsx")), ""),
            Some(SourceLanguage::Tsx)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Component.vue")), ""),
            Some(SourceLanguage::Vue)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.py")), ""),
            Some(SourceLanguage::Python)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("index.php")), ""),
            Some(SourceLanguage::Php)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Program.cs")), ""),
            Some(SourceLanguage::CSharp)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("app.rb")), ""),
            Some(SourceLanguage::Ruby)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("script.sh")), ""),
            Some(SourceLanguage::Shell)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("schema.graphql")), ""),
            Some(SourceLanguage::GraphQl)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Main.java")), ""),
            Some(SourceLanguage::Java)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Token.sol")), ""),
            Some(SourceLanguage::Solidity)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Main.kt")), ""),
            Some(SourceLanguage::Kotlin)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("App.swift")), ""),
            Some(SourceLanguage::Swift)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("app.ex")), ""),
            Some(SourceLanguage::Elixir)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("build.gradle")), ""),
            Some(SourceLanguage::Groovy)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.dart")), ""),
            Some(SourceLanguage::Dart)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Main.hs")), ""),
            Some(SourceLanguage::Haskell)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("app.erl")), ""),
            Some(SourceLanguage::Erlang)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.f90")), ""),
            Some(SourceLanguage::Fortran)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.jl")), ""),
            Some(SourceLanguage::Julia)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.lua")), ""),
            Some(SourceLanguage::Lua)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Main.scala")), ""),
            Some(SourceLanguage::Scala)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Module.vb")), ""),
            Some(SourceLanguage::VbNet)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.c")), ""),
            Some(SourceLanguage::CFamily)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("header.h")), ""),
            Some(SourceLanguage::CFamily)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.cpp")), ""),
            Some(SourceLanguage::CFamily)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("header.hpp")), ""),
            Some(SourceLanguage::CFamily)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("main.go")), ""),
            Some(SourceLanguage::Go)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("Cargo.toml")), ""),
            Some(SourceLanguage::Toml)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("config.yaml")), ""),
            Some(SourceLanguage::Yaml)
        );
        assert_eq!(
            detector.detect_language(Some(Path::new("config.yml")), ""),
            Some(SourceLanguage::Yaml)
        );
    }

    #[test]
    fn test_extension_detector_rejects_unsupported_languages() {
        let detector = ExtensionLanguageDetector;

        assert_eq!(detector.detect_language(Some(Path::new("README.md")), ""), None);
        assert_eq!(detector.detect_language(Some(Path::new("Dockerfile")), ""), None);
        assert_eq!(detector.detect_language(None, ""), None);
    }

    #[test]
    fn test_strip_noise_rust_comments_are_replaced_with_whitespace() {
        let input = "fn main() {\n    // comment\n    println!(\"hi\");\n}";
        let result = strip_noise(Some(Path::new("main.rs")), input);
        assert_eq!(result, "fn main() {\n    //        \n    println!(\"hi\");\n}");
    }

    #[test]
    fn test_strip_noise_non_rust_source_is_currently_unchanged() {
        let input = "console.log('hi'); // comment";
        let result = strip_noise(Some(Path::new("main.js")), input);
        assert_eq!(result, "console.log('hi'); //        ");
    }

    #[test]
    fn test_strip_noise_jsx_inline_comment_is_rewritten() {
        let input = "return <div>{/** comment */}</div>;";
        let result = strip_noise(Some(Path::new("view.jsx")), input);
        assert_eq!(result, "return <div>{//}</div>;");
    }

    #[test]
    fn test_strip_noise_python_comments_are_replaced_with_whitespace() {
        let input = "value = 1  # comment\nprint(value)";
        let result = strip_noise(Some(Path::new("main.py")), input);
        assert_eq!(result, "value = 1  #        \nprint(value)");
    }

    #[test]
    fn test_strip_noise_go_comments_are_replaced_with_whitespace() {
        let input = "package main\n\nfunc main() {\n    value := 1 // comment\n    _ = value\n}\n";
        let result = strip_noise(Some(Path::new("main.go")), input);
        assert_eq!(
            result,
            "package main\n\nfunc main() {\n    value := 1 //        \n    _ = value\n}\n"
        );
    }

    #[test]
    fn test_strip_noise_php_comments_are_replaced_with_whitespace() {
        let input = "<?php\n$value = 1; // comment\n";
        let result = strip_noise(Some(Path::new("index.php")), input);
        assert_eq!(result, "<?php\n$value = 1; //        \n");
    }

    #[test]
    fn test_strip_noise_c_family_comments_are_replaced_with_whitespace() {
        let input = "int main() {\n    int value = 1; // comment\n    return value;\n}\n";
        let result = strip_noise(Some(Path::new("main.c")), input);
        assert_eq!(
            result,
            "int main() {\n    int value = 1; //        \n    return value;\n}\n"
        );
    }

    #[test]
    fn test_strip_noise_cpp_block_comments_are_replaced_with_whitespace() {
        let input = "int value = /* note */ 1;\n";
        let result = strip_noise(Some(Path::new("main.cpp")), input);
        assert_eq!(result, "int value = /*      */ 1;\n");
    }

    #[test]
    fn test_strip_noise_csharp_comments_are_replaced_with_whitespace() {
        let input = "var value = 1; // comment\n";
        let result = strip_noise(Some(Path::new("Program.cs")), input);
        assert_eq!(result, "var value = 1; //        \n");
    }

    #[test]
    fn test_strip_noise_ruby_comments_are_replaced_with_whitespace() {
        let input = "value = 1 # comment\nputs value\n";
        let result = strip_noise(Some(Path::new("app.rb")), input);
        assert_eq!(result, "value = 1 #        \nputs value\n");
    }

    #[test]
    fn test_strip_noise_shell_comments_are_replaced_with_whitespace() {
        let input = "echo hi # comment\n";
        let result = strip_noise(Some(Path::new("script.sh")), input);
        assert_eq!(result, "echo hi #        \n");
    }

    #[test]
    fn test_strip_noise_graphql_comments_are_replaced_with_whitespace() {
        let input = "type Query {\n  me: User # comment\n}\n";
        let result = strip_noise(Some(Path::new("schema.graphql")), input);
        assert_eq!(result, "type Query {\n  me: User #        \n}\n");
    }

    #[test]
    fn test_strip_noise_java_comments_are_replaced_with_whitespace() {
        let input = "class Main { int value = 1; // comment\n}\n";
        let result = strip_noise(Some(Path::new("Main.java")), input);
        assert_eq!(result, "class Main { int value = 1; //        \n}\n");
    }

    #[test]
    fn test_strip_noise_solidity_comments_are_replaced_with_whitespace() {
        let input = "contract Token { uint value = 1; // comment\n}\n";
        let result = strip_noise(Some(Path::new("Token.sol")), input);
        assert_eq!(result, "contract Token { uint value = 1; //        \n}\n");
    }

    #[test]
    fn test_strip_noise_kotlin_comments_are_replaced_with_whitespace() {
        let input = "val value = 1 // comment\n";
        let result = strip_noise(Some(Path::new("Main.kt")), input);
        assert_eq!(result, "val value = 1 //        \n");
    }

    #[test]
    fn test_strip_noise_swift_comments_are_replaced_with_whitespace() {
        let input = "let value = 1 // comment\n";
        let result = strip_noise(Some(Path::new("App.swift")), input);
        assert_eq!(result, "let value = 1 //        \n");
    }

    #[test]
    fn test_strip_noise_elixir_comments_are_replaced_with_whitespace() {
        let input = "value = 1 # comment\nIO.puts(value)\n";
        let result = strip_noise(Some(Path::new("app.ex")), input);
        assert_eq!(result, "value = 1 #        \nIO.puts(value)\n");
    }

    #[test]
    fn test_strip_noise_groovy_comments_are_replaced_with_whitespace() {
        let input = "def value = 1 // comment\n";
        let result = strip_noise(Some(Path::new("build.gradle")), input);
        assert_eq!(result, "def value = 1 //        \n");
    }

    #[test]
    fn test_strip_noise_dart_comments_are_replaced_with_whitespace() {
        let input = "final value = 1; // comment\n";
        let result = strip_noise(Some(Path::new("main.dart")), input);
        assert_eq!(result, "final value = 1; //        \n");
    }

    #[test]
    fn test_strip_noise_haskell_comments_are_replaced_with_whitespace() {
        let input = "x = 1 -- comment\n";
        let result = strip_noise(Some(Path::new("Main.hs")), input);
        assert_eq!(result, "x = 1 --        \n");
    }

    #[test]
    fn test_strip_noise_erlang_comments_are_replaced_with_whitespace() {
        let input = "Value = 1. % comment\n";
        let result = strip_noise(Some(Path::new("app.erl")), input);
        assert_eq!(result, "Value = 1. %        \n");
    }

    #[test]
    fn test_strip_noise_fortran_comments_are_replaced_with_whitespace() {
        let input = "print *, \"hi\" ! comment\n";
        let result = strip_noise(Some(Path::new("main.f90")), input);
        assert_eq!(result, "print *, \"hi\" !        \n");
    }

    #[test]
    fn test_strip_noise_julia_comments_are_replaced_with_whitespace() {
        let input = "x = 1 # comment\n";
        let result = strip_noise(Some(Path::new("main.jl")), input);
        assert_eq!(result, "x = 1 #        \n");
    }

    #[test]
    fn test_strip_noise_lua_comments_are_replaced_with_whitespace() {
        let input = "local x = 1 -- comment\n";
        let result = strip_noise(Some(Path::new("main.lua")), input);
        assert_eq!(result, "local x = 1 --        \n");
    }

    #[test]
    fn test_strip_noise_scala_comments_are_replaced_with_whitespace() {
        let input = "val x = 1 // comment\n";
        let result = strip_noise(Some(Path::new("Main.scala")), input);
        assert_eq!(result, "val x = 1 //        \n");
    }

    #[test]
    fn test_strip_noise_vb_net_comments_are_replaced_with_whitespace() {
        let input = "Dim x = 1 ' comment\n";
        let result = strip_noise(Some(Path::new("Module.vb")), input);
        assert_eq!(result, "Dim x = 1 '        \n");
    }

    #[test]
    fn test_strip_noise_toml_comments_are_replaced_with_whitespace() {
        let input = "name = \"demo\" # comment\n";
        let result = strip_noise(Some(Path::new("Cargo.toml")), input);
        assert_eq!(result, "name = \"demo\" #        \n");
    }

    #[test]
    fn test_strip_noise_yaml_comments_are_replaced_with_whitespace() {
        let input = "name: demo # comment\n";
        let result = strip_noise(Some(Path::new("config.yaml")), input);
        assert_eq!(result, "name: demo #        \n");
    }

    #[test]
    fn test_strip_noise_vue_single_line_doc_comment_is_preserved() {
        let input = "const value = fn(/** keep */ arg);";
        let result = strip_noise(Some(Path::new("Component.vue")), input);
        assert_eq!(result, input);
    }
}
