use tree_sitter::{Node, Parser};

use super::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_dart_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_dart::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Dart,
        has_errors: root.has_error(),
        items,
    })
}

fn collect_items(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    if let Some(kind) = classify_node(node) {
        let name = node_name(node, source);
        if name.is_none() && matches!(kind, CodeItemKind::Function | CodeItemKind::Method) {
            return collect_children(node, source, items);
        }

        items.push(CodeItem {
            kind,
            name,
            range: node_range(node),
        });
    }

    collect_children(node, source, items);
}

fn collect_children(node: Node<'_>, source: &[u8], items: &mut Vec<CodeItem>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_items(child, source, items);
    }
}

fn classify_node(node: Node<'_>) -> Option<CodeItemKind> {
    match node.kind() {
        "import_or_export" => Some(CodeItemKind::Import),
        "class_declaration" | "mixin_declaration" | "extension_declaration" => {
            Some(CodeItemKind::Struct)
        }
        "enum_declaration" => Some(CodeItemKind::Enum),
        "function_signature" | "local_function_declaration" => {
            if has_class_ancestor(node) {
                Some(CodeItemKind::Method)
            } else {
                Some(CodeItemKind::Function)
            }
        }
        "method_signature" => Some(CodeItemKind::Method),
        "type_alias" => Some(CodeItemKind::TypeAlias),
        _ => None,
    }
}

fn has_class_ancestor(node: Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(current) = parent {
        if matches!(
            current.kind(),
            "class_declaration" | "mixin_declaration" | "extension_declaration"
        ) {
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
    fn inspects_dart_items() {
        let structure = inspect_dart_structure(
            "import 'dart:io';\nclass User { String greet() => 'hi'; }\nUser boot() => User();\n",
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
                (CodeItemKind::Method, Some("greet")),
                (CodeItemKind::Function, Some("boot")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
