use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_kotlin_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser.parse(content, None).ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Kotlin,
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
        "import_header" => Some(CodeItemKind::Import),
        "class_declaration" | "object_declaration" => Some(CodeItemKind::Struct),
        "function_declaration" => {
            if has_type_ancestor(node) {
                Some(CodeItemKind::Method)
            } else {
                Some(CodeItemKind::Function)
            }
        }
        "type_alias" => Some(CodeItemKind::TypeAlias),
        "property_declaration" => Some(CodeItemKind::Static),
        _ => None,
    }
}

fn has_type_ancestor(node: Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if matches!(current.kind(), "class_declaration" | "object_declaration") {
            return true;
        }
        parent = current.parent();
    }
    false
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .or_else(|| first_identifier(node))
        .and_then(|name| name.utf8_text(source).ok().map(str::to_string))
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "identifier" || node.kind() == "simple_identifier" {
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
    fn inspects_kotlin_items() {
        let structure = inspect_kotlin_structure(
            "class User { fun greet(): String { return \"hi\" } }\nfun boot(): User { return User() }\n",
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
                (CodeItemKind::Method, Some("greet")),
                (CodeItemKind::Function, Some("boot")),
            ]
        );
        // tree-sitter-kotlin-ng currently reports recovery nodes for compact
        // snippets even while extracting the declarations we need.
    }
}
