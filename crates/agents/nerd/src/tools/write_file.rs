//! Write file tool implementation.
//!
//! Provides functionality to write content to files.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use xoxo_core::tooling::{ErasedTool, Tool, ToolContext, ToolError, ToolRegistration, ToolSchema};

/// Error type for write_file operations
#[derive(Debug)]
pub enum WriteFileError {
    IoError(String),
    FileExists(String),
    CallbackError(String),
}

impl std::fmt::Display for WriteFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteFileError::IoError(msg) => write!(f, "IO error: {}", msg),
            WriteFileError::FileExists(path) => write!(f, "File already exists: {}", path),
            WriteFileError::CallbackError(msg) => write!(f, "Callback error: {}", msg),
        }
    }
}

/// Result type for write_file operations
pub type WriteFileResult = Result<String, WriteFileError>;

#[derive(Debug, Deserialize)]
struct WriteFileInput {
    file_path: String,
    content: String,
}

/// Tool implementation for writing files.
pub struct WriteFileTool;

impl WriteFileTool {
    pub fn new() -> Self {
        Self
    }

    /// Execute the write-file tool with contract-shaped arguments.
    pub async fn execute(
        &self,
        ctx: &ToolContext,
        file_path: &str,
        content: &str,
    ) -> Result<Value, ToolError> {
        let checksum = self
            .write_file_impl(file_path, content, None)
            .map_err(map_write_file_error)?;

        if let Some(exec_ctx) = &ctx.execution_context {
            exec_ctx
                .file_registry
                .lock()
                .await
                .upsert(file_path, checksum.clone());
        }

        Ok(json!({
            "message": format!("File saved: {file_path}"),
            "file_path": file_path,
            "exists": true,
            "md5": checksum,
            "line_count": content.lines().count(),
        }))
    }

    fn write_file_impl(
        &self,
        file_path: &str,
        content: &str,
        callback: Option<fn(&str, usize) -> Result<(), String>>,
    ) -> WriteFileResult {
        write_file_impl(file_path, content, callback)
    }
}

impl Tool for WriteFileTool {
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: "write_file".to_string(),
            description: "Write new content to a file on disk. Fails if the target file already exists.".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["file_path", "content"],
                "additionalProperties": false,
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to create, absolute or relative to the current working directory."
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file contents to write."
                    }
                }
            }),
        }
    }

    fn map_to_preview(&self, output: &Value) -> String {
        let file_path = output["file_path"].as_str();
        let checksum = output["md5"].as_str();

        match (file_path, checksum) {
            (Some(file_path), Some(checksum)) => {
                format!("File saved: {file_path} (MD5: {checksum})")
            }
            (Some(file_path), None) => format!("File saved: {file_path}"),
            _ => "File saved".to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<Value, ToolError> {
        let input: WriteFileInput = serde_json::from_value(input)
            .map_err(|err| ToolError::InvalidInput(err.to_string()))?;

        WriteFileTool::execute(self, ctx, &input.file_path, &input.content).await
    }
}

fn map_write_file_error(error: WriteFileError) -> ToolError {
    ToolError::ExecutionFailed(error.to_string())
}

/// Write file tool function.
///
/// # Arguments
///
/// * `file_path` - Path to the file (absolute or relative to PWD)
/// * `content` - Content to write to the file
/// * `callback` - Optional callback function to execute after successful write
///
/// # Returns
///
/// * `Ok(String)` - MD5 checksum of the persisted content
/// * `Err(WriteFileError)` - Error with reason on failure
fn write_file_impl(
    file_path: &str,
    content: &str,
    callback: Option<fn(&str, usize) -> Result<(), String>>,
) -> Result<String, WriteFileError> {
    // Check if file exists
    if Path::new(file_path).exists() {
        return Err(WriteFileError::FileExists(file_path.to_string()));
    }

    // Count lines in content
    let line_count = content.lines().count();

    // Write content to file
    match fs::write(file_path, content) {
        Ok(_) => {
            let checksum = format!("{:x}", md5::compute(content));

            // Execute callback if provided
            if let Some(cb) = callback {
                match cb(file_path, line_count) {
                    Ok(_) => Ok(checksum),
                    Err(e) => Err(WriteFileError::CallbackError(e)),
                }
            } else {
                Ok(checksum)
            }
        }
        Err(e) => Err(WriteFileError::IoError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tempfile::NamedTempFile;
    use xoxo_core::tooling::{Tool, ToolContext};

    static CALLBACK_CALLED: AtomicBool = AtomicBool::new(false);

    #[test]
    fn test_write_new_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        let result = WriteFileTool.write_file_impl(file_path_str, "Test content", None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), format!("{:x}", md5::compute("Test content")));
        
        let content = fs::read_to_string(file_path).unwrap();
        assert_eq!(content, "Test content");
    }

    #[test]
    fn test_write_with_callback() {
        fn callback(_path: &str, line_count: usize) -> Result<(), String> {
            CALLBACK_CALLED.store(true, Ordering::SeqCst);
            assert_eq!(line_count, 1);
            Ok(())
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        CALLBACK_CALLED.store(false, Ordering::SeqCst);
        let result = WriteFileTool.write_file_impl(file_path_str, "Test content", Some(callback));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), format!("{:x}", md5::compute("Test content")));
        assert!(CALLBACK_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_write_with_failing_callback() {
        fn callback(_path: &str, _line_count: usize) -> Result<(), String> {
            Err("Callback failed".to_string())
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        let result = WriteFileTool.write_file_impl(file_path_str, "Test content", Some(callback));
        assert!(matches!(result, Err(WriteFileError::CallbackError(_))));
    }

    #[test]
    fn test_write_with_multiline_callback() {
        fn callback(_path: &str, line_count: usize) -> Result<(), String> {
            assert_eq!(line_count, 3);
            Ok(())
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        let content = "Line 1\nLine 2\nLine 3";
        let result = WriteFileTool.write_file_impl(file_path_str, content, Some(callback));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), format!("{:x}", md5::compute(content)));
    }

    #[test]
    fn test_write_existing_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        
        // Write initial content
        fs::write(path, "Initial content").unwrap();
        
        // Try to write to existing file
        let result = WriteFileTool.write_file_impl(path, "New content", None);
        assert!(matches!(result, Err(WriteFileError::FileExists(_))));
    }

    #[test]
    fn test_tool_execute_from_contract() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        let output = futures::executor::block_on(Tool::execute(
            &WriteFileTool,
            &ToolContext {
                execution_context: None,
                spawner: None,
            },
            json!({
                "file_path": file_path_str,
                "content": "Line 1\nLine 2",
            }),
        ))
        .unwrap();

        assert_eq!(output["message"], format!("File saved: {file_path_str}"));
        assert_eq!(output["file_path"], file_path_str);
        assert_eq!(output["exists"], true);
        assert_eq!(output["md5"], format!("{:x}", md5::compute("Line 1\nLine 2")));
        assert_eq!(output["line_count"], 2);
        assert_eq!(fs::read_to_string(file_path).unwrap(), "Line 1\nLine 2");
    }

    #[test]
    fn test_map_to_preview_includes_checksum() {
        let preview = Tool::map_to_preview(&WriteFileTool, &json!({
            "file_path": "/tmp/example.txt",
            "md5": "abc123",
        }));

        assert_eq!(preview, "File saved: /tmp/example.txt (MD5: abc123)");
    }
}



inventory::submit! {
    ToolRegistration {
        name: "write_file",
        factory: || Arc::new(WriteFileTool::new()) as Arc<dyn ErasedTool>,
    }
}
