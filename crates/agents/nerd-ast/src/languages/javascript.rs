use tree_sitter::{Language, Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_javascript_structure(
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    inspect_ecmascript_structure(
        content,
        CodeLanguage::JavaScript,
        tree_sitter_javascript::LANGUAGE.into(),
    )
}

pub(crate) fn inspect_typescript_structure(
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    inspect_ecmascript_structure(
        content,
        CodeLanguage::TypeScript,
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    )
}

pub(crate) fn inspect_tsx_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    inspect_ecmascript_structure(
        content,
        CodeLanguage::Tsx,
        tree_sitter_typescript::LANGUAGE_TSX.into(),
    )
}

fn inspect_ecmascript_structure(
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
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language,
        has_errors: root.has_error(),
        items,
    })
}

fn collect_items(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    if let Some(kind) = classify_node(node) {
        items.push(CodeItem {
            kind,
            name: node_name(node, source),
            range: node_range(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(child, source, items);
    }
}

fn classify_node(node: Node<'_>) -> Option<CodeItemKind> {
    match node.kind() {
        "import_statement" => Some(CodeItemKind::Import),
        "function_declaration" | "generator_function_declaration" => Some(CodeItemKind::Function),
        "method_definition" | "method_signature" => Some(CodeItemKind::Method),
        "class_declaration" => Some(CodeItemKind::Struct),
        "interface_declaration" => Some(CodeItemKind::Trait),
        "enum_declaration" => Some(CodeItemKind::Enum),
        "type_alias_declaration" => Some(CodeItemKind::TypeAlias),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    name.utf8_text(source).ok().map(str::to_string)
}

fn node_range(node: Node<'_>) -> CodeRange {
    CodeRange {
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_javascript_items_in_source_order() {
        let structure = inspect_javascript_structure(
            "import fs from 'fs';\n\
             class User { greet() { return 'hi'; } }\n\
             function main() { return new User(); }\n",
        )
        .unwrap();

        let items = structure
            .items
            .iter()
            .map(|item| (item.kind, item.name.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                (CodeItemKind::Import, None),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, Some("greet")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }

    #[test]
    fn inspects_typescript_items_in_source_order() {
        let structure = inspect_typescript_structure(
            "import type { Config } from './config';\n\
             interface Named { name: string }\n\
             type Id = string;\n\
             enum State { Ready }\n\
             class User { greet(): string { return 'hi'; } }\n\
             function main(): User { return new User(); }\n",
        )
        .unwrap();

        let items = structure
            .items
            .iter()
            .map(|item| (item.kind, item.name.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                (CodeItemKind::Import, None),
                (CodeItemKind::Trait, Some("Named")),
                (CodeItemKind::TypeAlias, Some("Id")),
                (CodeItemKind::Enum, Some("State")),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, Some("greet")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }

    #[test]
    fn inspects_tsx_items() {
        let structure = inspect_tsx_structure(
            "type Props = { name: string };\n\
             function View(props: Props) { return <div>{props.name}</div>; }\n",
        )
        .unwrap();

        let items = structure
            .items
            .iter()
            .map(|item| (item.kind, item.name.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                (CodeItemKind::TypeAlias, Some("Props")),
                (CodeItemKind::Function, Some("View")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
