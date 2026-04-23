use std::path::Path;

use serde::{Deserialize, Serialize};

/// Source languages supported by AST-backed code inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeLanguage {
    /// Assembly source parsed with the Tree-sitter ASM grammar.
    Assembly,
    /// Bash source parsed with the Tree-sitter Bash grammar.
    Bash,
    /// C source parsed with the Tree-sitter C grammar.
    C,
    /// C# source parsed with the Tree-sitter C# grammar.
    CSharp,
    /// C++ source parsed with the Tree-sitter C++ grammar.
    Cpp,
    /// Dart source parsed with the Tree-sitter Dart grammar.
    Dart,
    /// Elixir source parsed with the Tree-sitter Elixir grammar.
    Elixir,
    /// Erlang source parsed with the Tree-sitter Erlang grammar.
    Erlang,
    /// Fortran source parsed with the Tree-sitter Fortran grammar.
    Fortran,
    /// Go source parsed with the Tree-sitter Go grammar.
    Go,
    /// GraphQL parsed with the Tree-sitter GraphQL grammar.
    Graphql,
    /// Groovy source parsed with the Tree-sitter Groovy grammar.
    Groovy,
    /// Haskell source parsed with the Tree-sitter Haskell grammar.
    Haskell,
    /// Java source parsed with the Tree-sitter Java grammar.
    Java,
    /// JavaScript source parsed with the Tree-sitter JavaScript grammar.
    JavaScript,
    /// JSON parsed with the Tree-sitter JSON grammar.
    Json,
    /// Julia source parsed with the Tree-sitter Julia grammar.
    Julia,
    /// Kotlin source parsed with the Tree-sitter Kotlin grammar.
    Kotlin,
    /// Lua source parsed with the Tree-sitter Lua grammar.
    Lua,
    /// MATLAB source parsed with the Tree-sitter MATLAB grammar.
    Matlab,
    /// Nix source parsed with the Tree-sitter Nix grammar.
    Nix,
    /// Objective-C source parsed with the Tree-sitter Objective-C grammar.
    ObjectiveC,
    /// Pascal source parsed with the Tree-sitter Pascal grammar.
    Pascal,
    /// Perl source parsed with the Tree-sitter Perl grammar.
    Perl,
    /// PHP source parsed with the Tree-sitter PHP grammar.
    Php,
    /// Protocol Buffers parsed with the Tree-sitter Proto grammar.
    Proto,
    /// Python source parsed with the Tree-sitter Python grammar.
    Python,
    /// PowerShell source parsed with the Tree-sitter PowerShell grammar.
    PowerShell,
    /// R source parsed with the Tree-sitter R grammar.
    R,
    /// Ruby source parsed with the Tree-sitter Ruby grammar.
    Ruby,
    /// Rust source parsed with the Tree-sitter Rust grammar.
    Rust,
    /// Scala source parsed with the Tree-sitter Scala grammar.
    Scala,
    /// Solidity source parsed with the Tree-sitter Solidity grammar.
    Solidity,
    /// Swift source parsed with the Tree-sitter Swift grammar.
    Swift,
    /// TOML parsed with the Tree-sitter TOML grammar.
    Toml,
    /// TypeScript source parsed with the Tree-sitter TypeScript grammar.
    TypeScript,
    /// TSX source parsed with the Tree-sitter TSX grammar.
    Tsx,
    /// YAML parsed with the Tree-sitter YAML grammar.
    Yaml,
    /// Zig source parsed with the Tree-sitter Zig grammar.
    Zig,
    /// VB.NET source parsed with the Tree-sitter VB.NET grammar.
    VbDotNet,
}

impl CodeLanguage {
    /// Parse a stable language name accepted by AST tools.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "assembly" | "asm" => Some(Self::Assembly),
            "bash" | "sh" => Some(Self::Bash),
            "c" => Some(Self::C),
            "c_sharp" | "csharp" | "cs" => Some(Self::CSharp),
            "cpp" | "c++" | "cc" | "cxx" => Some(Self::Cpp),
            "dart" => Some(Self::Dart),
            "elixir" | "ex" => Some(Self::Elixir),
            "erlang" | "erl" => Some(Self::Erlang),
            "fortran" | "f90" | "f95" | "f03" | "f08" => Some(Self::Fortran),
            "go" => Some(Self::Go),
            "graphql" | "gql" => Some(Self::Graphql),
            "groovy" | "gradle" => Some(Self::Groovy),
            "haskell" | "hs" => Some(Self::Haskell),
            "java" => Some(Self::Java),
            "javascript" | "js" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            "julia" | "jl" => Some(Self::Julia),
            "kotlin" | "kt" => Some(Self::Kotlin),
            "lua" => Some(Self::Lua),
            "matlab" | "m" => Some(Self::Matlab),
            "nix" => Some(Self::Nix),
            "objective_c" | "objc" | "obj-c" => Some(Self::ObjectiveC),
            "pascal" | "pas" => Some(Self::Pascal),
            "perl" | "pl" => Some(Self::Perl),
            "php" => Some(Self::Php),
            "proto" | "protobuf" => Some(Self::Proto),
            "python" | "py" => Some(Self::Python),
            "powershell" | "ps1" => Some(Self::PowerShell),
            "r" => Some(Self::R),
            "ruby" | "rb" => Some(Self::Ruby),
            "rust" | "rs" => Some(Self::Rust),
            "scala" | "sc" => Some(Self::Scala),
            "solidity" | "sol" => Some(Self::Solidity),
            "swift" => Some(Self::Swift),
            "toml" => Some(Self::Toml),
            "typescript" | "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "yaml" | "yml" => Some(Self::Yaml),
            "zig" => Some(Self::Zig),
            "vb_dotnet" | "vb" | "vbnet" => Some(Self::VbDotNet),
            _ => None,
        }
    }

    pub(crate) fn matches_path(self, file_path: &Path) -> bool {
        detect_language(file_path) == Some(self)
    }
}

pub(crate) fn detect_language(file_path: &Path) -> Option<CodeLanguage> {
    let extension = file_path.extension().and_then(|ext| ext.to_str())?;

    match extension {
        "asm" | "s" | "S" => Some(CodeLanguage::Assembly),
        "bash" | "sh" => Some(CodeLanguage::Bash),
        "c" | "h" => Some(CodeLanguage::C),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(CodeLanguage::Cpp),
        "cs" => Some(CodeLanguage::CSharp),
        "dart" => Some(CodeLanguage::Dart),
        "ex" | "exs" => Some(CodeLanguage::Elixir),
        "erl" | "hrl" => Some(CodeLanguage::Erlang),
        "f" | "f90" | "f95" | "f03" | "f08" | "for" => Some(CodeLanguage::Fortran),
        "go" => Some(CodeLanguage::Go),
        "graphql" | "gql" => Some(CodeLanguage::Graphql),
        "groovy" | "gradle" => Some(CodeLanguage::Groovy),
        "hs" | "lhs" => Some(CodeLanguage::Haskell),
        "java" => Some(CodeLanguage::Java),
        "js" | "jsx" | "mjs" | "cjs" => Some(CodeLanguage::JavaScript),
        "json" => Some(CodeLanguage::Json),
        "jl" => Some(CodeLanguage::Julia),
        "kt" | "kts" => Some(CodeLanguage::Kotlin),
        "lua" => Some(CodeLanguage::Lua),
        "m" => Some(CodeLanguage::Matlab),
        "nix" => Some(CodeLanguage::Nix),
        "mm" => Some(CodeLanguage::ObjectiveC),
        "pas" | "pp" | "p" => Some(CodeLanguage::Pascal),
        "pl" | "pm" => Some(CodeLanguage::Perl),
        "php" => Some(CodeLanguage::Php),
        "proto" => Some(CodeLanguage::Proto),
        "py" => Some(CodeLanguage::Python),
        "ps1" | "psm1" | "psd1" => Some(CodeLanguage::PowerShell),
        "r" | "R" => Some(CodeLanguage::R),
        "rb" => Some(CodeLanguage::Ruby),
        "rs" => Some(CodeLanguage::Rust),
        "scala" | "sc" => Some(CodeLanguage::Scala),
        "sol" => Some(CodeLanguage::Solidity),
        "swift" => Some(CodeLanguage::Swift),
        "toml" => Some(CodeLanguage::Toml),
        "ts" | "mts" | "cts" => Some(CodeLanguage::TypeScript),
        "tsx" => Some(CodeLanguage::Tsx),
        "yaml" | "yml" => Some(CodeLanguage::Yaml),
        "zig" => Some(CodeLanguage::Zig),
        "vb" => Some(CodeLanguage::VbDotNet),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_by_extension() {
        assert_eq!(
            detect_language(Path::new("src/lib.rs")),
            Some(CodeLanguage::Rust)
        );
    }

    #[test]
    fn detects_python_by_extension() {
        assert_eq!(
            detect_language(Path::new("src/main.py")),
            Some(CodeLanguage::Python)
        );
    }

    #[test]
    fn detects_go_by_extension() {
        assert_eq!(
            detect_language(Path::new("cmd/server/main.go")),
            Some(CodeLanguage::Go)
        );
    }

    #[test]
    fn detects_new_grammar_extensions() {
        assert_eq!(detect_language(Path::new("main.c")), Some(CodeLanguage::C));
        assert_eq!(
            detect_language(Path::new("index.js")),
            Some(CodeLanguage::JavaScript)
        );
        assert_eq!(
            detect_language(Path::new("index.ts")),
            Some(CodeLanguage::TypeScript)
        );
        assert_eq!(
            detect_language(Path::new("view.tsx")),
            Some(CodeLanguage::Tsx)
        );
        assert_eq!(detect_language(Path::new("app.rb")), Some(CodeLanguage::Ruby));
        assert_eq!(detect_language(Path::new("app.php")), Some(CodeLanguage::Php));
    }

    #[test]
    fn detects_second_batch_grammar_extensions() {
        assert_eq!(detect_language(Path::new("main.cpp")), Some(CodeLanguage::Cpp));
        assert_eq!(detect_language(Path::new("config.json")), Some(CodeLanguage::Json));
        assert_eq!(detect_language(Path::new("script.sh")), Some(CodeLanguage::Bash));
        assert_eq!(detect_language(Path::new("Program.cs")), Some(CodeLanguage::CSharp));
        assert_eq!(detect_language(Path::new("init.lua")), Some(CodeLanguage::Lua));
        assert_eq!(detect_language(Path::new("script.pl")), Some(CodeLanguage::Perl));
        assert_eq!(detect_language(Path::new("App.swift")), Some(CodeLanguage::Swift));
        assert_eq!(detect_language(Path::new("Cargo.toml")), Some(CodeLanguage::Toml));
        assert_eq!(detect_language(Path::new("values.yaml")), Some(CodeLanguage::Yaml));
    }

    #[test]
    fn detects_third_batch_grammar_extensions() {
        assert_eq!(detect_language(Path::new("Main.java")), Some(CodeLanguage::Java));
        assert_eq!(detect_language(Path::new("Main.hs")), Some(CodeLanguage::Haskell));
        assert_eq!(detect_language(Path::new("Main.kt")), Some(CodeLanguage::Kotlin));
        assert_eq!(detect_language(Path::new("model.m")), Some(CodeLanguage::Matlab));
        assert_eq!(detect_language(Path::new("analysis.R")), Some(CodeLanguage::R));
        assert_eq!(detect_language(Path::new("Main.scala")), Some(CodeLanguage::Scala));
        assert_eq!(detect_language(Path::new("server.erl")), Some(CodeLanguage::Erlang));
    }

    #[test]
    fn detects_fourth_batch_grammar_extensions() {
        assert_eq!(detect_language(Path::new("Main.groovy")), Some(CodeLanguage::Groovy));
        assert_eq!(detect_language(Path::new("solver.f90")), Some(CodeLanguage::Fortran));
        assert_eq!(detect_language(Path::new("app.ex")), Some(CodeLanguage::Elixir));
        assert_eq!(detect_language(Path::new("main.dart")), Some(CodeLanguage::Dart));
        assert_eq!(detect_language(Path::new("flake.nix")), Some(CodeLanguage::Nix));
        assert_eq!(detect_language(Path::new("script.ps1")), Some(CodeLanguage::PowerShell));
        assert_eq!(detect_language(Path::new("main.zig")), Some(CodeLanguage::Zig));
    }

    #[test]
    fn detects_last_batch_grammar_extensions() {
        assert_eq!(detect_language(Path::new("main.sol")), Some(CodeLanguage::Solidity));
        assert_eq!(
            detect_language(Path::new("schema.graphql")),
            Some(CodeLanguage::Graphql)
        );
        assert_eq!(detect_language(Path::new("query.gql")), Some(CodeLanguage::Graphql));
        assert_eq!(detect_language(Path::new("boot.asm")), Some(CodeLanguage::Assembly));
        assert_eq!(detect_language(Path::new("boot.S")), Some(CodeLanguage::Assembly));
        assert_eq!(detect_language(Path::new("api.proto")), Some(CodeLanguage::Proto));
    }

    #[test]
    fn rejects_unsupported_extensions() {
        assert_eq!(detect_language(Path::new("src/lib.txt")), None);
    }
}
