use tree_sitter::{Node, Parser};

use super::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_c_sharp_structure(
    content: &str,
) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::CSharp,
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
        "using_directive" => Some(CodeItemKind::Import),
        "namespace_declaration" | "file_scoped_namespace_declaration" => {
            Some(CodeItemKind::Module)
        }
        "class_declaration" | "struct_declaration" | "record_declaration" => {
            Some(CodeItemKind::Struct)
        }
        "interface_declaration" => Some(CodeItemKind::Trait),
        "enum_declaration" => Some(CodeItemKind::Enum),
        "method_declaration" | "constructor_declaration" => Some(CodeItemKind::Method),
        _ => None,
    }
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
    fn inspects_c_sharp_items_in_source_order() {
        let structure = inspect_c_sharp_structure(
            "using System;\n\
             namespace App;\n\
             interface Named {}\n\
             class User { public string Greet() { return \"hi\"; } }\n",
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
                (CodeItemKind::Module, Some("App")),
                (CodeItemKind::Trait, Some("Named")),
                (CodeItemKind::Struct, Some("User")),
                (CodeItemKind::Method, Some("Greet")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
