//! Read file tool implementation.
//!
//! Provides functionality to read file contents with optional line range.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

/// Import the noise stripper function
use crate::coding::noise_stripper::strip_noise;

/// Error type for read_file operations
#[derive(Debug)]
pub enum ReadFileError {
    FileNotFound(String),
    InvalidLineRange { requested: String, total_lines: usize },
    IoError(String),
}

impl std::fmt::Display for ReadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadFileError::FileNotFound(path) => write!(f, "File not found: {}", path),
            ReadFileError::InvalidLineRange { requested, total_lines } => {
                write!(f, "Invalid line range '{}' for file with {} lines", requested, total_lines)
            }
            ReadFileError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

/// Result type for read_file operations
pub type ReadFileResult = Result<(String, usize), ReadFileError>;

#[derive(Debug, Deserialize)]
struct ReadFileInput {
    file_path: String,
    #[serde(default)]
    line_range: Option<String>,
    #[serde(default)]
    with_noise: bool,
}

/// Tool implementation for reading files.
pub struct ReadFileTool;

impl ReadFileTool {

    pub fn new() -> Self {
        Self
    }

    /// Execute the read-file tool with contract-shaped arguments.
    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        line_range: Option<&str>,
        with_noise: bool,
    ) -> Result<Value, ToolError> {
        let (content, total_lines) = self
            .read_file_impl(file_path, line_range, with_noise)
            .map_err(map_read_file_error)?;

        if let Some(exec_ctx) = &ctx.execution_context {
            let checksum = read_file_md5(file_path).map_err(map_read_file_error)?;
            exec_ctx.file_registry.lock().await.upsert(file_path, checksum);
        }

        Ok(json!({
            "content": content,
            "total_lines": total_lines,
        }))
    }

    fn read_file_impl(
        &self,
        file_path: &str,
        line_range: Option<&str>,
        with_noise: bool,
    ) -> ReadFileResult {
        read_file_impl(file_path, line_range, with_noise)
    }
}

impl Tool for ReadFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "read_file".to_string(),
            description: "Read a file from disk, optionally limited to a 1-indexed inclusive line range. Can optionally include comments and other noise.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file, absolute or relative to the current working directory."
                    },
                    "line_range": {
                        "type": "string",
                        "description": "Optional 1-indexed inclusive line range in the format 'start:end'."
                    },
                    "with_noise": {
                        "type": "boolean",
                        "description": "When true, return the raw file including comments and other noise. Defaults to false."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let total_lines = output["total_lines"].as_u64();
        let returned_lines = output["content"]
            .as_str()
            .map(|content| content.lines().count());

        match (total_lines, returned_lines) {
            (Some(total_lines), Some(returned_lines)) => {
                format!(
                    "Read file successfully ({returned_lines} line(s) returned, {total_lines} total)"
                )
            }
            (Some(total_lines), None) => {
                format!("Read file successfully ({total_lines} total lines)")
            }
            _ => "Read file successfully".to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let input: ReadFileInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        ReadFileTool::execute(
            self,
            ctx,
            &input.file_path,
            input.line_range.as_deref(),
            input.with_noise,
        )
        .await
    }
}

fn map_read_file_error(error: ReadFileError) -> ToolError {
    match error {
        ReadFileError::InvalidLineRange { .. } => ToolError::InvalidInput(error.to_string()),
        ReadFileError::FileNotFound(_) | ReadFileError::IoError(_) => {
            ToolError::ExecutionFailed(error.to_string())
        }
    }
}

/// Read file tool function.
///
/// # Arguments
///
/// * `file_path` - Path to the file (absolute or relative to PWD)
/// * `line_range` - Optional line range in format "start:end" (1-indexed, inclusive)
/// * `with_noise` - If truthy, strips the file of comments before returning
///
/// # Returns
///
/// * `Ok((content, total_lines))` - File content and total line count on success
/// * `Err(ReadFileError)` - Error with reason on failure
///
/// # Examples
///
/// ```rust
/// use nerd::tools::read_file::ReadFileTool;
///
/// # let _ = ReadFileTool;
/// ```
fn read_file_impl(
    file_path: &str,
    line_range: Option<&str>,
    with_noise: bool,
) -> ReadFileResult {
    // Check if file exists
    if !Path::new(file_path).exists() {
        return Err(ReadFileError::FileNotFound(file_path.to_string()));
    }

    // Read file content
    let content = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => return Err(ReadFileError::IoError(e.to_string())),
    };

    let total_lines = content.lines().count();

    // Apply noise stripping to full content if with_noise is false
    let processed_content = if !with_noise {
        strip_noise(Some(Path::new(file_path)), &content)
    } else {
        content
    };

    // Parse line range if provided
    if let Some(range_str) = line_range {
        let parts: Vec<&str> = range_str.split(':').collect();
        if parts.len() != 2 {
            return Err(ReadFileError::InvalidLineRange {
                requested: range_str.to_string(),
                total_lines,
            });
        }

        let start: usize = match parts[0].parse() {
            Ok(val) => val,
            Err(_) => {
                return Err(ReadFileError::InvalidLineRange {
                    requested: range_str.to_string(),
                    total_lines,
                });
            }
        };

        let end: usize = match parts[1].parse() {
            Ok(val) => val,
            Err(_) => {
                return Err(ReadFileError::InvalidLineRange {
                    requested: range_str.to_string(),
                    total_lines,
                });
            }
        };

        // Validate range (convert to 0-indexed for processing)
        if start == 0 || end == 0 || start > end || start > total_lines {
            return Err(ReadFileError::InvalidLineRange {
                requested: range_str.to_string(),
                total_lines,
            });
        }

        // Adjust end to not exceed total lines
        let end = end.min(total_lines);

        // Extract requested lines from processed content
        let selected_lines: Vec<&str> = processed_content
            .lines()
            .skip(start - 1)
            .take(end - start + 1)
            .collect();

        let result_content = selected_lines.join("\n");
        return Ok((result_content, total_lines));
    }

    // Return full processed content if no range specified
    Ok((processed_content, total_lines))
}

fn read_file_md5(file_path: &str) -> Result<String, ReadFileError> {
    let content = fs::read_to_string(file_path).map_err(|e| ReadFileError::IoError(e.to_string()))?;
    Ok(format!("{:x}", md5::compute(content)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use serde_json::json;
    use tempfile::NamedTempFile;
    use xoxo_core::tooling::{Tool, ToolContext};

    #[test]
    fn test_read_full_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1\nLine 2\nLine 3").unwrap();
        let path = file.path().to_str().unwrap();

        let result = ReadFileTool.read_file_impl(path, None, false);
        assert!(result.is_ok());
        let (content, total) = result.unwrap();
        assert_eq!(total, 3);
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 3"));
    }

    #[test]
    fn test_read_line_range() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();
        let path = file.path().to_str().unwrap();

        let result = ReadFileTool.read_file_impl(path, Some("2:3"), false);
        assert!(result.is_ok());
        let (content, total) = result.unwrap();
        assert_eq!(total, 4);
        assert!(!content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("Line 3"));
        assert!(!content.contains("Line 4"));
    }

    #[test]
    fn test_partial_range() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1\nLine 2").unwrap();
        let path = file.path().to_str().unwrap();

        // Request range that partially exists
        let result = ReadFileTool.read_file_impl(path, Some("1:5"), false);
        assert!(result.is_ok());
        let (content, total) = result.unwrap();
        assert_eq!(total, 2);
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
    }

    #[test]
    fn test_invalid_range() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1\nLine 2").unwrap();
        let path = file.path().to_str().unwrap();

        let result = ReadFileTool.read_file_impl(path, Some("5:10"), false);
        assert!(matches!(result, Err(ReadFileError::InvalidLineRange { .. })));
    }

    #[test]
    fn test_file_not_found() {
        let result = ReadFileTool.read_file_impl("nonexistent.txt", None, false);
        assert!(matches!(result, Err(ReadFileError::FileNotFound(_))));
    }

    #[test]
    fn test_tool_execute_from_contract() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1\nLine 2").unwrap();
        let path = file.path().to_str().unwrap();

        let output = futures::executor::block_on(Tool::execute(
            &ReadFileTool,
            &ToolContext {
                execution_context: None,
                spawner: None,
            },
            json!({
                "file_path": path,
                "line_range": "1:1",
                "with_noise": true,
            }),
        ))
        .unwrap();

        assert_eq!(output["content"], "Line 1");
        assert_eq!(output["total_lines"], 2);
    }

    #[test]
    fn test_map_to_preview_redacts_file_contents() {
        let preview = Tool::map_to_preview(&ReadFileTool, &json!({
            "content": "secret line 1\nsecret line 2",
            "total_lines": 10,
        }));

        assert_eq!(preview, "Read file successfully (2 line(s) returned, 10 total)");
        assert!(!preview.contains("secret"));
    }
}

inventory::submit! {
    ToolRegistration {
        name: "read_file",
        factory: || Arc::new(ReadFileTool::new()) as Arc<dyn ErasedTool>,
    }
}
