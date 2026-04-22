use std::path::PathBuf;
use std::sync::Arc;

use agentix::tooling::{
    ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::coding::{
    CodeLanguage, CodeStructureError, FindSymbolOptions, find_symbol,
};

#[derive(Debug, Deserialize)]
struct FindSymbolInput {
    name: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    root: Option<String>,
}

/// Tool implementation for AST-backed exact symbol search.
pub struct FindSymbolTool;

impl FindSymbolTool {
    /// Create a new exact symbol search tool.
    pub fn new() -> Self {
        Self
    }
}

impl Tool for FindSymbolTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "find_symbol".to_string(),
            description: "Find exact symbol definitions under a root using deterministic AST parsers. Supports Rust, Python, Go, JavaScript, TypeScript, Ruby, PHP, C, C++, Bash, C#, Lua, Perl, Swift, JSON, TOML, and YAML initially.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["name"],
                "additionalProperties": false,
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact symbol name to find."
                    },
                    "language": {
                        "type": "string",
                        "enum": ["rust", "python", "go", "javascript", "typescript", "tsx", "ruby", "php", "c", "cpp", "bash", "csharp", "lua", "perl", "swift", "json", "toml", "yaml"],
                        "description": "Optional language filter. When omitted, all supported AST languages are searched."
                    },
                    "root": {
                        "type": "string",
                        "description": "Optional root directory to search. Defaults to the current working directory."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let name = output["name"].as_str().unwrap_or("symbol");
        let hit_count = output["hit_count"].as_u64().unwrap_or(0);
        format!("Found {hit_count} definition(s) for {name}")
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let input: FindSymbolInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;
        let language = parse_language(input.language.as_deref())?;
        let root = input.root.map(PathBuf::from);
        let result = find_symbol(FindSymbolOptions {
            name: input.name,
            language,
            root,
        })
        .map_err(map_symbol_error)?;

        serde_json::to_value(result).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

fn parse_language(language: Option<&str>) -> Result<Option<CodeLanguage>, ToolError> {
    let Some(language) = language else {
        return Ok(None);
    };

    CodeLanguage::from_name(language)
        .map(Some)
        .ok_or_else(|| ToolError::InvalidInput(format!("unsupported language: {language}")))
}

fn map_symbol_error(error: CodeStructureError) -> ToolError {
    match error {
        CodeStructureError::UnsupportedLanguage(_) => ToolError::InvalidInput(error.to_string()),
        CodeStructureError::ParserConfiguration(_) | CodeStructureError::ParseFailed => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

inventory::submit! {
    ToolRegistration {
        name: "find_symbol",
        factory: || Arc::new(FindSymbolTool::new()) as Arc<dyn ErasedTool>,
    }
}
