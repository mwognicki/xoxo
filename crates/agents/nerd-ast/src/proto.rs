use tree_sitter::{Node, Parser};

use super::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_proto_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_proto::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Proto,
        has_errors: root.has_error(),
        items,
    })
}

fn collect_items(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    if let Some(kind) = classify_node(node) {
        if let Some(name) = node_name(node, source) {
            items.push(CodeItem {
                kind,
                name: Some(name),
                range: node_range(node),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(child, source, items);
    }
}

fn classify_node(node: Node<'_>) -> Option<CodeItemKind> {
    match node.kind() {
        "message" => Some(CodeItemKind::Struct),
        "enum" => Some(CodeItemKind::Enum),
        "service" => Some(CodeItemKind::Trait),
        "rpc" => Some(CodeItemKind::Method),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    first_named_child(node, source, name_node_kind(node.kind()))
}

fn name_node_kind(kind: &str) -> &str {
    match kind {
        "message" => "message_name",
        "enum" => "enum_name",
        "service" => "service_name",
        "rpc" => "rpc_name",
        _ => "identifier",
    }
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
    fn inspects_proto_items() {
        let structure = inspect_proto_structure(
            "syntax = \"proto3\";\nmessage User { string id = 1; }\nservice Users { rpc Boot (User) returns (User); }\n",
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
                (CodeItemKind::Trait, Some("Users")),
                (CodeItemKind::Method, Some("Boot")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
