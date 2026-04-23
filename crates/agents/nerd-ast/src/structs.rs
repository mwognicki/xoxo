use serde::Serialize;

use super::CodeLanguage;

/// Structural outline for a parsed source file.
#[derive(Debug, Clone, Serialize)]
pub struct CodeStructure {
    /// Language parser used to produce this outline.
    pub language: CodeLanguage,
    /// Whether Tree-sitter found syntax errors in the parsed file.
    pub has_errors: bool,
    /// Top-level and method-level items discovered in source order.
    pub items: Vec<CodeItem>,
}

/// A single structural item discovered in source code.
#[derive(Debug, Clone, Serialize)]
pub struct CodeItem {
    /// Kind of source construct represented by this item.
    pub kind: CodeItemKind,
    /// Best-effort symbol name for named constructs.
    pub name: Option<String>,
    /// One-indexed line and byte range covered by this item.
    pub range: CodeRange,
}

/// Kind of source construct represented by a [`CodeItem`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeItemKind {
    /// Import or use declaration.
    Import,
    /// Function outside an implementation block.
    Function,
    /// Function inside an implementation block.
    Method,
    /// Struct declaration.
    Struct,
    /// Enum declaration.
    Enum,
    /// Trait declaration.
    Trait,
    /// Implementation block.
    Impl,
    /// Module declaration.
    Module,
    /// Type alias.
    TypeAlias,
    /// Constant declaration.
    Const,
    /// Static declaration.
    Static,
    /// Macro definition.
    Macro,
}

/// Source range for a structural item.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CodeRange {
    /// One-indexed starting line.
    pub start_line: usize,
    /// One-indexed ending line.
    pub end_line: usize,
    /// Starting byte offset.
    pub start_byte: usize,
    /// Ending byte offset.
    pub end_byte: usize,
}
