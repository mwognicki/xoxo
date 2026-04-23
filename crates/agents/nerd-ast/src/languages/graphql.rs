use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_graphql_structure(
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_graphql::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Graphql,
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
        "object_type_definition" | "input_object_type_definition" => Some(CodeItemKind::Struct),
        "interface_type_definition" => Some(CodeItemKind::Trait),
        "enum_type_definition" => Some(CodeItemKind::Enum),
        "scalar_type_definition" => Some(CodeItemKind::TypeAlias),
        "schema_definition" => Some(CodeItemKind::Module),
        "operation_definition" | "fragment_definition" => Some(CodeItemKind::Function),
        "directive_definition" => Some(CodeItemKind::Macro),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    first_named_child(node, source, "name")
        .or_else(|| first_named_child(node, source, "fragment_name"))
}

fn first_named_child(node: Node<'_>, source: &[u8], kind: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return child.utf8_text(source).ok().map(str::to_string);
        }
    }
    None
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
    fn inspects_graphql_items() {
        let structure = inspect_graphql_structure(
            "type User { id: ID! }\ninterface Node { id: ID! }\nfragment userFields on User { id }\nquery boot { viewer { id } }\n",
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
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Trait, Some("Node")),
                (CodeItemKind::Function, Some("userFields")),
                (CodeItemKind::Function, Some("boot")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
