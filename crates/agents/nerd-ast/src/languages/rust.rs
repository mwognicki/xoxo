use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_rust_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Rust,
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
        "use_declaration" => Some(CodeItemKind::Import),
        "function_item" => {
            if has_impl_ancestor(node) {
                Some(CodeItemKind::Method)
            } else {
                Some(CodeItemKind::Function)
            }
        }
        "struct_item" => Some(CodeItemKind::Struct),
        "enum_item" => Some(CodeItemKind::Enum),
        "trait_item" => Some(CodeItemKind::Trait),
        "impl_item" => Some(CodeItemKind::Impl),
        "mod_item" => Some(CodeItemKind::Module),
        "type_item" => Some(CodeItemKind::TypeAlias),
        "const_item" => Some(CodeItemKind::Const),
        "static_item" => Some(CodeItemKind::Static),
        "macro_definition" => Some(CodeItemKind::Macro),
        _ => None,
    }
}

fn has_impl_ancestor(node: Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if current.kind() == "impl_item" {
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
    fn inspects_rust_items_in_source_order() {
        let structure = inspect_rust_structure(
            "use std::fmt;\n\
             struct User;\n\
             enum State { Ready }\n\
             trait Named { fn name(&self) -> &str; }\n\
             impl User { fn new() -> Self { Self } }\n\
             fn main() {}\n",
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
                (CodeItemKind::Enum, Some("State")),
                (CodeItemKind::Trait, Some("Named")),
                (CodeItemKind::Impl, None),
                (CodeItemKind::Method, Some("new")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
