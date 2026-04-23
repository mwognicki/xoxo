use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_elixir_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_elixir::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Elixir,
        has_errors: root.has_error(),
        items,
    })
}

fn collect_items(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    if is_named_call(node, source, "defmodule") {
        items.push(CodeItem {
            kind: CodeItemKind::Module,
            name: second_identifier(node, source),
            range: node_range(node),
        });
    } else if is_named_call(node, source, "def") || is_named_call(node, source, "defp") {
        items.push(CodeItem {
            kind: CodeItemKind::Function,
            name: second_identifier(node, source),
            range: node_range(node),
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(child, source, items);
    }
}

fn is_named_call(node: Node<'_>, source: &[u8], name: &str) -> bool {
    node.kind() == "call" && first_identifier_text(node, source).as_deref() == Some(name)
}

fn first_identifier_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    first_identifier(node).and_then(|identifier| {
        identifier
            .utf8_text(source)
            .ok()
            .map(|text| text.trim_start_matches(':').to_string())
    })
}

fn second_identifier(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut identifiers = Vec::new();
    collect_identifiers(node, source, &mut identifiers);
    identifiers.get(1).cloned()
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "identifier" | "alias" | "atom") {
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

fn collect_identifiers(node: Node<'_>, source: &[u8], identifiers: &mut Vec<String>) {
    if matches!(node.kind(), "identifier" | "alias" | "atom") {
        if let Ok(text) = node.utf8_text(source) {
            identifiers.push(text.trim_start_matches(':').to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifiers(child, source, identifiers);
    }
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
    fn inspects_elixir_items() {
        let structure = inspect_elixir_structure(
            "defmodule App do\n  def boot(x) do\n    x\n  end\nend\n",
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
                (CodeItemKind::Module, Some("App")),
                (CodeItemKind::Function, Some("boot")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
