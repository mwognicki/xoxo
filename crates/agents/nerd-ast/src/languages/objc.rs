use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_objc_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_objc::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::ObjectiveC,
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
        "class_interface" | "class_implementation" | "class_declaration" => {
            Some(CodeItemKind::Struct)
        }
        "protocol_declaration" => Some(CodeItemKind::Trait),
        "method_declaration" | "method_definition" => Some(CodeItemKind::Method),
        "function_definition" => Some(CodeItemKind::Function),
        "struct_specifier" => Some(CodeItemKind::Struct),
        "enum_specifier" => Some(CodeItemKind::Enum),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .or_else(|| first_identifier(node))
        .and_then(|name| name.utf8_text(source).ok().map(str::to_string))
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "method_declaration" || node.kind() == "method_definition" {
        return find_by_kind(node, "method_identifier");
    }

    if matches!(
        node.kind(),
        "identifier" | "type_identifier" | "method_identifier"
    ) {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(identifier) = first_identifier(child) {
            return Some(identifier);
        }
    }
    None
}

fn find_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_by_kind(child, kind) {
            return Some(found);
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
    fn inspects_objc_items() {
        let structure = inspect_objc_structure(
            "@interface User\n- (NSString *)greet;\n@end\n@implementation User\n- (NSString *)greet { return @\"hi\"; }\n@end\n",
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
                (CodeItemKind::Method, None),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, None),
            ]
        );
        assert!(!structure.has_errors);
    }
}
