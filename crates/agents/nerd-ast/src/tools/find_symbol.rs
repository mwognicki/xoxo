use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Serialize;

use crate::{
    CodeItem, CodeLanguage, CodeRange, CodeStructureError, detect_language,
    inspect_code_structure,
};

/// Options for deterministic AST-backed symbol search.
#[derive(Debug, Clone)]
pub struct FindSymbolOptions {
    /// Exact symbol name to find.
    pub name: String,
    /// Optional language filter. Unsupported files are skipped when absent.
    pub language: Option<CodeLanguage>,
    /// Root path to search. Defaults to the current working directory.
    pub root: Option<PathBuf>,
}

/// Result of an AST-backed symbol search.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolSearchResult {
    /// Exact symbol name searched for.
    pub name: String,
    /// Language filter used for the search.
    pub language: Option<CodeLanguage>,
    /// Root path searched.
    pub root: String,
    /// Number of matching symbols.
    pub hit_count: usize,
    /// Matching symbols in deterministic file and source order.
    pub hits: Vec<SymbolHit>,
}

/// A single matching symbol discovered in source code.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolHit {
    /// File path relative to the searched root.
    pub file_path: String,
    /// Language parser that found the symbol.
    pub language: CodeLanguage,
    /// Matched structural item.
    pub item: CodeItem,
}

/// Find exact symbol definitions under a root using supported AST parsers.
///
/// # Errors
///
/// Returns [`CodeStructureError::ParserConfiguration`] when the current
/// directory cannot be determined, and propagates parser configuration errors
/// from supported language parsers. Files that cannot be read or are unsupported
/// are skipped.
pub fn find_symbol(
    options: FindSymbolOptions,
) -> Result<SymbolSearchResult, CodeStructureError> {
    let root = match options.root {
        Some(root) => root,
        None => std::env::current_dir()
            .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?,
    };

    let mut hits = Vec::new();

    for result in WalkBuilder::new(&root).standard_filters(true).build() {
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
        if !is_supported_by_filter(path, options.language) {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let structure = match inspect_code_structure(path, &content) {
            Ok(structure) => structure,
            Err(CodeStructureError::UnsupportedLanguage(_)) => continue,
            Err(error) => return Err(error),
        };

        for item in structure.items {
            if item.name.as_deref() == Some(options.name.as_str()) {
                hits.push(SymbolHit {
                    file_path: relative_to(&root, path),
                    language: structure.language,
                    item,
                });
            }
        }
    }

    hits.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| compare_ranges(&left.item.range, &right.item.range))
    });

    Ok(SymbolSearchResult {
        name: options.name,
        language: options.language,
        root: root.to_string_lossy().into_owned(),
        hit_count: hits.len(),
        hits,
    })
}

fn is_supported_by_filter(path: &Path, language: Option<CodeLanguage>) -> bool {
    match language {
        Some(language) => language.matches_path(path),
        None => detect_language(path).is_some(),
    }
}

fn compare_ranges(left: &CodeRange, right: &CodeRange) -> std::cmp::Ordering {
    left.start_line
        .cmp(&right.start_line)
        .then_with(|| left.start_byte.cmp(&right.start_byte))
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
    fn finds_rust_and_python_symbols_under_root() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("lib.rs"), "struct User;\nfn build() {}\n")
            .unwrap();
        fs::write(
            temp.path().join("app.py"),
            "class User:\n    def build(self):\n        return self\n",
        )
        .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "User".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language, hit.item.name.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("app.py", CodeLanguage::Python, Some("User")),
                ("lib.rs", CodeLanguage::Rust, Some("User")),
            ]
        );
        assert_eq!(result.hit_count, 2);
    }

    #[test]
    fn filters_symbols_by_language() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("lib.rs"), "fn build() {}\n").unwrap();
        fs::write(
            temp.path().join("app.py"),
            "def build():\n    return None\n",
        )
        .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "build".to_string(),
            language: Some(CodeLanguage::Python),
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        assert_eq!(result.hit_count, 1);
        assert_eq!(result.hits[0].file_path, "app.py");
        assert_eq!(result.hits[0].language, CodeLanguage::Python);
    }

    #[test]
    fn finds_go_symbols() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("main.go"),
            "package main\n\ntype User struct{}\n\nfunc NewUser() User { return User{} }\n",
        )
        .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "NewUser".to_string(),
            language: Some(CodeLanguage::Go),
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        assert_eq!(result.hit_count, 1);
        assert_eq!(result.hits[0].file_path, "main.go");
        assert_eq!(result.hits[0].language, CodeLanguage::Go);
    }

    #[test]
    fn finds_symbols_in_new_tree_sitter_grammars() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("main.c"), "int boot(void) { return 0; }\n").unwrap();
        fs::write(temp.path().join("app.js"), "function boot() { return 0; }\n").unwrap();
        fs::write(temp.path().join("app.ts"), "function boot(): number { return 0; }\n")
            .unwrap();
        fs::write(temp.path().join("app.rb"), "def boot\n  0\nend\n").unwrap();
        fs::write(temp.path().join("app.php"), "<?php\nfunction boot() { return 0; }\n")
            .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("app.js", CodeLanguage::JavaScript),
                ("app.php", CodeLanguage::Php),
                ("app.rb", CodeLanguage::Ruby),
                ("app.ts", CodeLanguage::TypeScript),
                ("main.c", CodeLanguage::C),
            ]
        );
    }

    #[test]
    fn finds_symbols_in_second_batch_tree_sitter_grammars() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("main.cpp"), "int boot() { return 0; }\n").unwrap();
        fs::write(temp.path().join("script.sh"), "boot() {\n  echo hi\n}\n").unwrap();
        fs::write(
            temp.path().join("Program.cs"),
            "class App { public void boot() {} }\n",
        )
        .unwrap();
        fs::write(temp.path().join("init.lua"), "function boot()\nend\n").unwrap();
        fs::write(temp.path().join("script.pl"), "sub boot { return 1; }\n").unwrap();
        fs::write(temp.path().join("App.swift"), "func boot() -> Int { 0 }\n").unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("App.swift", CodeLanguage::Swift),
                ("Program.cs", CodeLanguage::CSharp),
                ("init.lua", CodeLanguage::Lua),
                ("main.cpp", CodeLanguage::Cpp),
                ("script.pl", CodeLanguage::Perl),
                ("script.sh", CodeLanguage::Bash),
            ]
        );
    }

    #[test]
    fn finds_symbols_in_third_batch_tree_sitter_grammars() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Main.java"),
            "class App { void boot() {} }\n",
        )
        .unwrap();
        fs::write(temp.path().join("Main.hs"), "boot x = x + 1\n").unwrap();
        fs::write(temp.path().join("Main.kt"), "fun boot(): Int { return 1 }\n").unwrap();
        fs::write(temp.path().join("model.m"), "function y = boot(x)\ny = x;\nend\n")
            .unwrap();
        fs::write(temp.path().join("analysis.R"), "boot <- function(x) { x }\n").unwrap();
        fs::write(temp.path().join("Main.scala"), "def boot = 1\n").unwrap();
        fs::write(temp.path().join("server.erl"), "boot() -> ok.\n").unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("Main.hs", CodeLanguage::Haskell),
                ("Main.java", CodeLanguage::Java),
                ("Main.kt", CodeLanguage::Kotlin),
                ("Main.scala", CodeLanguage::Scala),
                ("analysis.R", CodeLanguage::R),
                ("model.m", CodeLanguage::Matlab),
                ("server.erl", CodeLanguage::Erlang),
            ]
        );
    }

    #[test]
    fn finds_symbols_in_fourth_batch_tree_sitter_grammars() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("Main.groovy"),
            "class App { String boot() { 'hi' } }\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("solver.f90"),
            "function boot(x) result(y)\ninteger :: x, y\ny = x\nend function boot\n",
        )
        .unwrap();
        fs::write(temp.path().join("app.ex"), "defmodule App do\n  def boot(x), do: x\nend\n")
            .unwrap();
        fs::write(temp.path().join("main.dart"), "boot() => 1;\n").unwrap();
        fs::write(temp.path().join("flake.nix"), "{ boot = x: x; }\n").unwrap();
        fs::write(temp.path().join("script.ps1"), "function boot { 1 }\n").unwrap();
        fs::write(temp.path().join("main.zig"), "pub fn boot() void {}\n").unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("Main.groovy", CodeLanguage::Groovy),
                ("app.ex", CodeLanguage::Elixir),
                ("flake.nix", CodeLanguage::Nix),
                ("main.dart", CodeLanguage::Dart),
                ("main.zig", CodeLanguage::Zig),
                ("script.ps1", CodeLanguage::PowerShell),
                ("solver.f90", CodeLanguage::Fortran),
            ]
        );
    }

    #[test]
    fn finds_symbols_in_final_tree_sitter_grammar_batch() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("main.jl"),
            "module App\nfunction boot(x)\n  x\nend\nend\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("main.pas"),
            "program Boot;\nbegin\nend.\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("main.mm"),
            "@interface Boot\n@end\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("Main.vb"),
            "Module Boot\nEnd Module\n",
        )
        .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "Boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("Main.vb", CodeLanguage::VbDotNet),
                ("main.mm", CodeLanguage::ObjectiveC),
                ("main.pas", CodeLanguage::Pascal),
            ]
        );

        let julia = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: Some(CodeLanguage::Julia),
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();
        assert_eq!(julia.hit_count, 1);
        assert_eq!(julia.hits[0].file_path, "main.jl");
    }

    #[test]
    fn finds_symbols_in_last_tree_sitter_grammar_batch() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("vault.sol"),
            "contract Vault {\nfunction boot() public {}\n}\n",
        )
        .unwrap();
        fs::write(temp.path().join("schema.graphql"), "query boot { viewer { id } }\n")
            .unwrap();
        fs::write(temp.path().join("boot.asm"), "boot:\n  retq\n").unwrap();
        fs::write(
            temp.path().join("api.proto"),
            "syntax = \"proto3\";\nservice Api { rpc boot (Req) returns (Req); }\nmessage Req {}\n",
        )
        .unwrap();

        let result = find_symbol(FindSymbolOptions {
            name: "boot".to_string(),
            language: None,
            root: Some(temp.path().to_path_buf()),
        })
        .unwrap();

        let hits = result
            .hits
            .iter()
            .map(|hit| (hit.file_path.as_str(), hit.language))
            .collect::<Vec<_>>();

        assert_eq!(
            hits,
            vec![
                ("api.proto", CodeLanguage::Proto),
                ("boot.asm", CodeLanguage::Assembly),
                ("schema.graphql", CodeLanguage::Graphql),
                ("vault.sol", CodeLanguage::Solidity),
            ]
        );
    }
}
