use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_php_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_php::LANGUAGE_PHP.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Php,
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
        "namespace_use_declaration" => Some(CodeItemKind::Import),
        "function_definition" => Some(CodeItemKind::Function),
        "method_declaration" => Some(CodeItemKind::Method),
        "class_declaration" => Some(CodeItemKind::Struct),
        "interface_declaration" | "trait_declaration" => Some(CodeItemKind::Trait),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let name = node.child_by_field_name("name")?;
    name.utf8_text(source).ok().map(trim_php_name).map(str::to_string)
}

fn trim_php_name(name: &str) -> &str {
    name.strip_prefix('$').unwrap_or(name)
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
    fn inspects_php_items_in_source_order() {
        let structure = inspect_php_structure(
            "<?php\n\
             use App\\Config;\n\
             interface Named {}\n\
             trait Loggable {}\n\
             class User { public function greet() { return 'hi'; } }\n\
             function main() { return new User(); }\n",
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
                (CodeItemKind::Trait, Some("Named")),
                (CodeItemKind::Trait, Some("Loggable")),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, Some("greet")),
                (CodeItemKind::Function, Some("main")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
