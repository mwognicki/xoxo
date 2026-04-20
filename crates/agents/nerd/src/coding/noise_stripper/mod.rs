//! Noise stripper module.
//!
//! Provides functionality to strip noise from supported source code files.

use std::path::Path;

mod go;
mod javascript;
mod python;
mod rust;
mod toml;
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
