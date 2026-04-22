use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

const CUSTOM_TOML_SYNTAX: &str = include_str!("../syntaxes/TOML.sublime-syntax");
const CUSTOM_RUST_SYNTAX: &str = include_str!("../syntaxes/RUST.sublime-syntax");

fn custom_toml_syntax_set() -> &'static SyntaxSet {
    static CUSTOM_TOML_SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

    CUSTOM_TOML_SYNTAX_SET.get_or_init(|| {
        let mut builder = SyntaxSetBuilder::new();
        builder.add_plain_text_syntax();
        let syntax = SyntaxDefinition::load_from_str(CUSTOM_TOML_SYNTAX, true, Some("TOML"))
            .expect("bundled TOML sublime syntax should parse");
        builder.add(syntax);
        builder.build()
    })
}

fn custom_rust_syntax_set() -> &'static SyntaxSet {
    static CUSTOM_RUST_SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

    CUSTOM_RUST_SYNTAX_SET.get_or_init(|| {
        let mut builder = SyntaxSetBuilder::new();
        builder.add_plain_text_syntax();
        let syntax = SyntaxDefinition::load_from_str(CUSTOM_RUST_SYNTAX, true, Some("RUST"))
            .expect("bundled RUST sublime syntax should parse");
        builder.add(syntax);
        builder.build()
    })
}

pub fn highlight_syntax(extension: &str, file_content: &str) -> String {
    let ps = match extension {
        "toml" | "tml" => custom_toml_syntax_set(),
        "rs" => custom_rust_syntax_set(),
        "py" => &sublime_syntaxes::extra_syntax_set().clone(),
        _ => &sublime_syntaxes::extra_syntax_set().clone(),
    };
    let ts = ThemeSet::load_defaults();

    let Some(syntax) = ps.find_syntax_by_extension(extension) else {
        return file_content.to_string();
    };

    let mut h = HighlightLines::new(syntax, &ts.themes["InspiredGitHub"]);
    let mut output = vec![];
    for line in LinesWithEndings::from(file_content) {
        let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        output.push(escaped);
    }
    output.concat()
}

#[cfg(test)]
mod tests {
    use super::custom_toml_syntax_set;

    #[test]
    fn bundled_toml_syntax_is_available_by_extension() {
        let syntax = custom_toml_syntax_set().find_syntax_by_extension("toml");
        assert!(syntax.is_some());
    }
}
