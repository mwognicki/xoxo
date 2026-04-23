use tree_sitter::{Language, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::CodeStructure,
};

pub(crate) fn inspect_json_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    inspect_data_structure(content, CodeLanguage::Json, tree_sitter_json::LANGUAGE.into())
}

pub(crate) fn inspect_toml_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    inspect_data_structure(content, CodeLanguage::Toml, tree_sitter_toml_ng::LANGUAGE.into())
}

pub(crate) fn inspect_yaml_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    inspect_data_structure(content, CodeLanguage::Yaml, tree_sitter_yaml::LANGUAGE.into())
}

fn inspect_data_structure(
    content: &str,
    language: CodeLanguage,
    tree_sitter_language: Language,
) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_language)
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();

    Ok(CodeStructure {
        language,
        has_errors: root.has_error(),
        items: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_data_languages_without_symbols() {
        let json = inspect_json_structure("{\"name\":\"xoxo\"}").unwrap();
        let toml = inspect_toml_structure("name = \"xoxo\"\n").unwrap();
        let yaml = inspect_yaml_structure("name: xoxo\n").unwrap();

        assert_eq!(json.language, CodeLanguage::Json);
        assert_eq!(toml.language, CodeLanguage::Toml);
        assert_eq!(yaml.language, CodeLanguage::Yaml);
        assert!(json.items.is_empty());
        assert!(toml.items.is_empty());
        assert!(yaml.items.is_empty());
        assert!(!json.has_errors);
        assert!(!toml.has_errors);
        assert!(!yaml.has_errors);
    }
}
