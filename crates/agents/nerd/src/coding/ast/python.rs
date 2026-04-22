use tree_sitter::{Node, Parser};

use super::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_python_structure(
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Python,
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
        "import_statement" | "import_from_statement" => Some(CodeItemKind::Import),
        "function_definition" => {
            if has_class_ancestor(node) {
                Some(CodeItemKind::Method)
            } else {
                Some(CodeItemKind::Function)
            }
        }
        "class_definition" => Some(CodeItemKind::Struct),
        _ => None,
    }
}

fn has_class_ancestor(node: Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == "class_definition" {
            return true;
        }
        parent = current.parent();
    }
    false
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
    fn inspects_python_items_in_source_order() {
        let structure = inspect_python_structure(
            "import os\n\
             from pathlib import Path\n\n\
             class User:\n\
             \x20\x20\x20\x20def __init__(self, name):\n\
             \x20\x20\x20\x20\x20\x20\x20\x20self.name = name\n\n\
             def main():\n\
             \x20\x20\x20\x20return User('Ada')\n",
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
                (CodeItemKind::Import, Some("os")),
                (CodeItemKind::Import, Some("Path")),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, Some("__init__")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
