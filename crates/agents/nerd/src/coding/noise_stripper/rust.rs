//! Rust-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    String,
    Char,
    LineComment,
    BlockComment,
    RawString { hashes: usize },
}

/// Strip comments from Rust source while preserving overall text shape.
///
/// Line count is preserved, comment delimiters remain in place, and removed
/// comment content is replaced with spaces.
pub fn strip_rust_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut block_depth = 0usize;

    while i < bytes.len() {
        match state {
            State::Code => {
                if let Some((prefix_len, hashes)) = raw_string_prefix(&bytes[i..]) {
                    out.push_str(&content[i..i + prefix_len]);
                    i += prefix_len;
                    state = State::RawString { hashes };
                    continue;
                }

                if starts_with(bytes, i, b"//") {
                    out.push('/');
                    out.push('/');
                    i += 2;
                    if let Some(&marker) = bytes.get(i) {
                        if marker == b'/' || marker == b'!' {
                            out.push(marker as char);
                            i += 1;
                        }
                    }
                    state = State::LineComment;
                    continue;
                }

                if starts_with(bytes, i, b"/*") {
                    out.push('/');
                    out.push('*');
                    i += 2;
                    if let Some(&marker) = bytes.get(i) {
                        if marker == b'*' || marker == b'!' {
                            out.push(marker as char);
                            i += 1;
                        }
                    }
                    block_depth = 1;
                    state = State::BlockComment;
                    continue;
                }

                match bytes[i] {
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::String;
                    }
                    b'\'' if looks_like_char_literal(bytes, i) => {
                        out.push('\'');
                        i += 1;
                        state = State::Char;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::String => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    continue;
                }

                if bytes[i] == b'"' {
                    state = State::Code;
                }

                i += 1;
            }
            State::Char => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    continue;
                }

                if bytes[i] == b'\'' {
                    state = State::Code;
                }

                i += 1;
            }
            State::LineComment => {
                if bytes[i] == b'\n' {
                    out.push('\n');
                    i += 1;
                    state = State::Code;
                } else {
                    out.push(' ');
                    i += 1;
                }
            }
            State::BlockComment => {
                if starts_with(bytes, i, b"/*") {
                    out.push('/');
                    out.push('*');
                    i += 2;
                    block_depth += 1;
                    continue;
                }

                if starts_with(bytes, i, b"*/") {
                    out.push('*');
                    out.push('/');
                    i += 2;
                    block_depth -= 1;
                    if block_depth == 0 {
                        state = State::Code;
                    }
                    continue;
                }

                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            State::RawString { hashes } => {
                out.push(bytes[i] as char);
                if bytes[i] == b'"' && has_raw_string_terminator(bytes, i, hashes) {
                    i += 1;
                    for _ in 0..hashes {
                        out.push('#');
                        i += 1;
                    }
                    state = State::Code;
                    continue;
                }
                i += 1;
            }
        }
    }

    out
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate == needle)
}

fn raw_string_prefix(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.is_empty() {
        return None;
    }

    let prefix_len = match bytes[0] {
        b'r' => 1,
        b'b' if bytes.get(1) == Some(&b'r') => 2,
        _ => return None,
    };

    let mut hashes = 0;
    while bytes.get(prefix_len + hashes) == Some(&b'#') {
        hashes += 1;
    }

    if bytes.get(prefix_len + hashes) == Some(&b'"') {
        Some((prefix_len + hashes + 1, hashes))
    } else {
        None
    }
}

fn has_raw_string_terminator(bytes: &[u8], quote_index: usize, hashes: usize) -> bool {
    for offset in 0..hashes {
        if bytes.get(quote_index + 1 + offset) != Some(&b'#') {
            return false;
        }
    }
    true
}

fn looks_like_char_literal(bytes: &[u8], start: usize) -> bool {
    let Some(&next) = bytes.get(start + 1) else {
        return false;
    };

    if next == b'\\' {
        return bytes.get(start + 3) == Some(&b'\'');
    }

    next != b'\'' && next != b'\n' && bytes.get(start + 2) == Some(&b'\'')
}

#[cfg(test)]
mod tests {
    use super::strip_rust_noise;

    #[test]
    fn strips_line_comments() {
        let input = "let x = 1; // comment\nlet y = 2;";
        let output = strip_rust_noise(input);
        assert_eq!(output, "let x = 1; //        \nlet y = 2;");
    }

    #[test]
    fn strips_doc_comments() {
        let input = "/// docs\nfn main() {}";
        let output = strip_rust_noise(input);
        assert_eq!(output, "///     \nfn main() {}");
    }

    #[test]
    fn strips_inner_doc_comments() {
        let input = "//! docs\nfn main() {}";
        let output = strip_rust_noise(input);
        assert_eq!(output, "//!     \nfn main() {}");
    }

    #[test]
    fn strips_block_comments_and_preserves_newlines() {
        let input = "fn main() { /* one\ntwo */ }";
        let output = strip_rust_noise(input);
        assert_eq!(output, "fn main() { /*    \n    */ }");
    }

    #[test]
    fn strips_nested_block_comments() {
        let input = "/* outer /* inner */ outer */";
        let output = strip_rust_noise(input);
        assert_eq!(output, "/*       /*       */       */");
    }

    #[test]
    fn keeps_comment_markers_inside_strings() {
        let input = "let s = \"// not comment /* nope */\";";
        let output = strip_rust_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_raw_strings() {
        let input = "let s = r#\"// not comment\"#;";
        let output = strip_rust_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_lifetimes_unchanged() {
        let input = "fn borrow<'a>(value: &'a str) -> &'a str { value }";
        let output = strip_rust_noise(input);
        assert_eq!(output, input);
    }
}
