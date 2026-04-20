use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLanguage {
    Python,
    /// JavaScript or TypeScript, executed by Deno.
    JsTs,
    /// Bash shell script.
    Shell,
}