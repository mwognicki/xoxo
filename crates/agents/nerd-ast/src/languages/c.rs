use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_c_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::C,
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
        "function_definition" => Some(CodeItemKind::Function),
        "struct_specifier" => Some(CodeItemKind::Struct),
        "enum_specifier" => Some(CodeItemKind::Enum),
        "type_definition" => Some(CodeItemKind::TypeAlias),
        "declaration" => Some(CodeItemKind::Static),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "function_definition" => node
            .child_by_field_name("declarator")
            .and_then(|declarator| find_identifier(declarator, source)),
        "struct_specifier" | "enum_specifier" => node
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source).ok().map(str::to_string)),
        "type_definition" | "declaration" => node
            .child_by_field_name("declarator")
            .and_then(|declarator| find_identifier(declarator, source)),
        _ => None,
    }
}

fn find_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "identifier" || node.kind() == "type_identifier" {
        return node.utf8_text(source).ok().map(str::to_string);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(identifier) = find_identifier(child, source) {
            return Some(identifier);
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
    fn inspects_c_items_in_source_order() {
        let structure = inspect_c_structure(
            "typedef int UserId;\n\
             struct User { int id; };\n\
             enum State { Ready };\n\
             static int enabled = 1;\n\
             int main(void) { return enabled; }\n",
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
                (CodeItemKind::TypeAlias, Some("UserId")),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Enum, Some("State")),
                (CodeItemKind::Static, Some("enabled")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
