use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_nix_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_nix::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Nix,
        has_errors: root.has_error(),
        items,
    })
}

fn collect_items(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    if node.kind() == "binding" {
        items.push(CodeItem {
            kind: if contains_function_expression(node) {
                CodeItemKind::Function
            } else {
                CodeItemKind::Static
            },
            name: node_name(node, source),
            range: node_range(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(child, source, items);
    }
}

fn contains_function_expression(node: Node<'_>) -> bool {
    if node.kind() == "function_expression" {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_function_expression(child) {
            return true;
        }
    }
    false
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    first_identifier(node).and_then(|name| name.utf8_text(source).ok().map(str::to_string))
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "identifier" {
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
    fn inspects_nix_items() {
        let structure = inspect_nix_structure("{ boot = x: x; value = 1; }\n").unwrap();
        let items = structure
            .items
            .iter()
            .map(|item| (item.kind, item.name.as_deref()))
            .collect::<Vec<_>>();

        assert_eq!(
            items,
            vec![
                (CodeItemKind::Function, Some("boot")),
                (CodeItemKind::Static, Some("value")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
