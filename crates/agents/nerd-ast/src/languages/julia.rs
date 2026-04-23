use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_julia_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_julia::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Julia,
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
        "module_definition" => Some(CodeItemKind::Module),
        "function_definition" => Some(CodeItemKind::Function),
        "struct_definition" | "primitive_definition" => Some(CodeItemKind::Struct),
        "abstract_definition" => Some(CodeItemKind::Trait),
        "macro_definition" => Some(CodeItemKind::Macro),
        "assignment" => Some(CodeItemKind::Static),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .or_else(|| first_identifier(node))
        .and_then(|name| name.utf8_text(source).ok().map(str::to_string))
}

fn first_identifier(node: Node<'_>) -> Option<Node<'_>> {
    if matches!(node.kind(), "identifier" | "macro_identifier") {
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
    fn inspects_julia_items() {
        let structure = inspect_julia_structure(
            "module App\nstruct User\nend\nfunction boot(x)\n  x\nend\nend\n",
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
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Function, Some("boot")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
