use std::path::Path;

use serde::{Deserialize, Serialize};

/// Source languages supported by AST-backed code inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeLanguage {
    /// Bash source parsed with the Tree-sitter Bash grammar.
    Bash,
    /// C source parsed with the Tree-sitter C grammar.
    C,
    /// C# source parsed with the Tree-sitter C# grammar.
    CSharp,
    /// C++ source parsed with the Tree-sitter C++ grammar.
    Cpp,
    /// Go source parsed with the Tree-sitter Go grammar.
    Go,
    /// JavaScript source parsed with the Tree-sitter JavaScript grammar.
    JavaScript,
    /// JSON parsed with the Tree-sitter JSON grammar.
    Json,
    /// Lua source parsed with the Tree-sitter Lua grammar.
    Lua,
    /// Perl source parsed with the Tree-sitter Perl grammar.
    Perl,
    /// PHP source parsed with the Tree-sitter PHP grammar.
    Php,
    /// Python source parsed with the Tree-sitter Python grammar.
    Python,
    /// Ruby source parsed with the Tree-sitter Ruby grammar.
    Ruby,
    /// Rust source parsed with the Tree-sitter Rust grammar.
    Rust,
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
}

impl CodeLanguage {
    /// Parse a stable language name accepted by AST tools.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bash" | "sh" => Some(Self::Bash),
            "c" => Some(Self::C),
            "c_sharp" | "csharp" | "cs" => Some(Self::CSharp),
            "cpp" | "c++" | "cc" | "cxx" => Some(Self::Cpp),
            "go" => Some(Self::Go),
            "javascript" | "js" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            "lua" => Some(Self::Lua),
            "perl" | "pl" => Some(Self::Perl),
            "php" => Some(Self::Php),
            "python" | "py" => Some(Self::Python),
            "ruby" | "rb" => Some(Self::Ruby),
            "rust" | "rs" => Some(Self::Rust),
            "swift" => Some(Self::Swift),
            "toml" => Some(Self::Toml),
            "typescript" | "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "yaml" | "yml" => Some(Self::Yaml),
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
        "bash" | "sh" => Some(CodeLanguage::Bash),
        "c" | "h" => Some(CodeLanguage::C),
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Some(CodeLanguage::Cpp),
        "cs" => Some(CodeLanguage::CSharp),
        "go" => Some(CodeLanguage::Go),
        "js" | "jsx" | "mjs" | "cjs" => Some(CodeLanguage::JavaScript),
        "json" => Some(CodeLanguage::Json),
        "lua" => Some(CodeLanguage::Lua),
        "pl" | "pm" => Some(CodeLanguage::Perl),
        "php" => Some(CodeLanguage::Php),
        "py" => Some(CodeLanguage::Python),
        "rb" => Some(CodeLanguage::Ruby),
        "rs" => Some(CodeLanguage::Rust),
        "swift" => Some(CodeLanguage::Swift),
        "toml" => Some(CodeLanguage::Toml),
        "ts" | "mts" | "cts" => Some(CodeLanguage::TypeScript),
        "tsx" => Some(CodeLanguage::Tsx),
        "yaml" | "yml" => Some(CodeLanguage::Yaml),
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
    fn rejects_unsupported_extensions() {
        assert_eq!(detect_language(Path::new("src/lib.txt")), None);
    }
}
