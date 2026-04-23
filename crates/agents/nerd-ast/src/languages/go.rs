use tree_sitter::{Node, Parser};

use crate::{
    CodeStructureError,
    language::CodeLanguage,
    structs::{CodeItem, CodeItemKind, CodeRange, CodeStructure},
};

pub(crate) fn inspect_go_structure(content: &str) -> Result<CodeStructure, CodeStructureError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|err| CodeStructureError::ParserConfiguration(err.to_string()))?;

    let tree = parser
        .parse(content, None)
        .ok_or(CodeStructureError::ParseFailed)?;
    let root = tree.root_node();
    let mut items = Vec::new();

    collect_items(root, content.as_bytes(), &mut items);

    Ok(CodeStructure {
        language: CodeLanguage::Go,
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
        "import_declaration" => Some(CodeItemKind::Import),
        "function_declaration" => Some(CodeItemKind::Function),
        "method_declaration" => Some(CodeItemKind::Method),
        "type_declaration" => Some(CodeItemKind::TypeAlias),
        "const_declaration" => Some(CodeItemKind::Const),
        "var_declaration" => Some(CodeItemKind::Static),
        _ => None,
    }
}

fn node_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == "type_declaration" {
        return first_child_name(node, source, "type_spec");
    }

    let name = node.child_by_field_name("name")?;
    name.utf8_text(source).ok().map(str::to_string)
}

fn first_child_name(node: Node<'_>, source: &[u8], child_kind: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != child_kind {
            continue;
        }
        let name = child.child_by_field_name("name")?;
        return name.utf8_text(source).ok().map(str::to_string);
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
    fn inspects_go_items_in_source_order() {
        let structure = inspect_go_structure(
            "package main\n\n\
             import \"fmt\"\n\n\
             const version = \"1\"\n\
             var enabled = true\n\n\
             type User struct { Name string }\n\n\
             func NewUser(name string) User { return User{Name: name} }\n\n\
             func (u User) Greet() string { return fmt.Sprintf(\"hi %s\", u.Name) }\n",
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
                (CodeItemKind::Const, None),
                (CodeItemKind::Static, None),
                (CodeItemKind::TypeAlias, Some("User")),
                (CodeItemKind::Function, Some("NewUser")),
                (CodeItemKind::Method, Some("Greet")),
            ]
        );
        assert!(!structure.has_errors);
    }
}
