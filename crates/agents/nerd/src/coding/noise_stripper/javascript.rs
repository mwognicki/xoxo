//! JavaScript and TypeScript-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaScriptFlavor {
    Standard,
    Jsx,
    Vue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    Template,
    Regex,
    LineComment,
    BlockComment,
}

/// Strip comments from JavaScript and TypeScript source while preserving text shape.
///
/// Line count is preserved, comment delimiters remain in place, and removed
/// comment content is replaced with spaces.
pub fn strip_javascript_noise(content: &str, flavor: JavaScriptFlavor) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => {
                if matches!(flavor, JavaScriptFlavor::Jsx)
                    && starts_with(bytes, i, b"{/**")
                    && is_single_line_jsx_comment(bytes, i)
                {
                    let end = find_single_line_jsx_comment_end(bytes, i)
                        .expect("checked by is_single_line_jsx_comment");
                    out.push_str("{//}");
                    i = end + 1;
                    continue;
                }

                if matches!(flavor, JavaScriptFlavor::Jsx)
                    && starts_with(bytes, i, b"/**")
                    && is_single_line_block_comment(bytes, i)
                {
                    out.push_str("/** . */");
                    i = find_single_line_block_comment_end(bytes, i)
                        .expect("checked by is_single_line_block_comment")
                        + 1;
                    continue;
                }

                if starts_with(bytes, i, b"//") {
                    out.push('/');
                    out.push('/');
                    i += 2;
                    state = State::LineComment;
                    continue;
                }

                if starts_with(bytes, i, b"/*") {
                    if matches!(flavor, JavaScriptFlavor::Vue)
                        && starts_with(bytes, i, b"/**")
                        && is_single_line_block_comment(bytes, i)
                    {
                        out.push('/');
                        out.push('*');
                        out.push('*');
                        i += 3;
                        while i < bytes.len() {
                            if starts_with(bytes, i, b"*/") {
                                out.push('*');
                                out.push('/');
                                i += 2;
                                break;
                            }
                            if bytes[i] == b'\n' {
                                break;
                            }
                            push_char_at(content, &mut out, &mut i);
                        }
                        continue;
                    }

                    out.push('/');
                    out.push('*');
                    i += 2;
                    state = State::BlockComment;
                    continue;
                }

                match bytes[i] {
                    b'\'' => {
                        out.push('\'');
                        i += 1;
                        state = State::SingleQuote;
                    }
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::DoubleQuote;
                    }
                    b'`' => {
                        out.push('`');
                        i += 1;
                        state = State::Template;
                    }
                    b'/' if can_start_regex(bytes, i) => {
                        out.push('/');
                        i += 1;
                        state = State::Regex;
                    }
                    _ => {
                        push_char_at(content, &mut out, &mut i);
                    }
                }
            }
            State::SingleQuote => {
                push_with_escape(content, bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
            }
            State::DoubleQuote => {
                push_with_escape(content, bytes, &mut out, &mut i, b'"', State::Code, &mut state);
            }
            State::Template => {
                push_char_at(content, &mut out, &mut i);
                let current = bytes[i - 1];
                if current == b'\\' {
                    if i < bytes.len() {
                        push_char_at(content, &mut out, &mut i);
                    }
                    continue;
                }

                if current == b'`' {
                    state = State::Code;
                }
            }
            State::Regex => {
                push_char_at(content, &mut out, &mut i);
                let current = bytes[i - 1];
                if current == b'\\' {
                    if i < bytes.len() {
                        push_char_at(content, &mut out, &mut i);
                    }
                    continue;
                }

                if current == b'[' {
                    while i < bytes.len() {
                        push_char_at(content, &mut out, &mut i);
                        let class_current = bytes[i - 1];
                        if class_current == b'\\' {
                            if i < bytes.len() {
                                push_char_at(content, &mut out, &mut i);
                            }
                        } else if class_current == b']' {
                            break;
                        }
                    }
                    continue;
                }

                if current == b'/' {
                    state = State::Code;
                }
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
                if starts_with(bytes, i, b"*/") {
                    out.push('*');
                    out.push('/');
                    i += 2;
                    state = State::Code;
                    continue;
                }

                if bytes[i] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
        }
    }

    out
}

fn push_with_escape(
    content: &str,
    bytes: &[u8],
    out: &mut String,
    index: &mut usize,
    terminator: u8,
    next_state: State,
    state: &mut State,
) {
    push_char_at(content, out, index);
    let current = bytes[*index - 1];
    if current == b'\\' {
        if *index < bytes.len() {
            push_char_at(content, out, index);
        }
        return;
    }

    if current == terminator {
        *state = next_state;
    }
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate == needle)
}

fn can_start_regex(bytes: &[u8], slash_index: usize) -> bool {
    if starts_with(bytes, slash_index, b"//") || starts_with(bytes, slash_index, b"/*") {
        return false;
    }

    let mut cursor = slash_index;
    while cursor > 0 {
        cursor -= 1;
        match bytes[cursor] {
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            b'(' | b'[' | b'{' | b',' | b';' | b':' | b'=' | b'!' | b'?' | b'&' | b'|'
            | b'^' | b'~' | b'+' | b'-' | b'*' | b'%' | b'<' | b'>' => return true,
            _ => return false,
        }
    }

    true
}

fn is_single_line_jsx_comment(bytes: &[u8], start_index: usize) -> bool {
    find_single_line_jsx_comment_end(bytes, start_index).is_some()
}

fn find_single_line_jsx_comment_end(bytes: &[u8], start_index: usize) -> Option<usize> {
    let mut i = start_index + 5;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            return None;
        }
        if starts_with(bytes, i, b"*/}") {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}

fn is_single_line_block_comment(bytes: &[u8], start_index: usize) -> bool {
    find_single_line_block_comment_end(bytes, start_index).is_some()
}

fn find_single_line_block_comment_end(bytes: &[u8], start_index: usize) -> Option<usize> {
    let mut i = start_index + 3;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            return None;
        }
        if starts_with(bytes, i, b"*/") {
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

fn push_char_at(content: &str, out: &mut String, index: &mut usize) {
    let ch = content[*index..]
        .chars()
        .next()
        .expect("index always points to a valid char boundary");
    out.push(ch);
    *index += ch.len_utf8();
}

#[cfg(test)]
mod tests {
    use super::{strip_javascript_noise, JavaScriptFlavor};

    #[test]
    fn strips_line_comments() {
        let input = "const x = 1; // comment\nconst y = 2;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, "const x = 1; //        \nconst y = 2;");
    }

    #[test]
    fn strips_block_comments() {
        let input = "const x = /* note */ 1;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, "const x = /*      */ 1;");
    }

    #[test]
    fn keeps_comment_markers_inside_strings() {
        let input = "const s = \"// not comment /* nope */\";";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_single_quotes() {
        let input = "const s = '// not comment';";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_template_literals() {
        let input = "const s = `// not comment`;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_regex_literals() {
        let input = "const re = /a\\/\\/b\\/\\*c/;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, input);
    }

    #[test]
    fn strips_typescript_comments() {
        let input = "const value: string = 'x'; // comment";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Standard);
        assert_eq!(output, "const value: string = 'x'; //        ");
    }

    #[test]
    fn rewrites_single_line_jsx_comments() {
        let input = "return <div>{/** comment */}</div>;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Jsx);
        assert_eq!(output, "return <div>{//}</div>;");
    }

    #[test]
    fn collapses_single_line_jsx_doc_comments() {
        let input = "/** Renders a single line. */\nconst value = 1;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Jsx);
        assert_eq!(output, "/** . */\nconst value = 1;");
    }

    #[test]
    fn preserves_single_line_vue_doc_comments() {
        let input = "const value = fn(/** keep */ arg);";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Vue);
        assert_eq!(output, input);
    }

    #[test]
    fn still_strips_multiline_vue_block_comments() {
        let input = "const value = /* note\nmore */ arg;";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Vue);
        assert_eq!(output, "const value = /*     \n     */ arg;");
    }

    #[test]
    fn preserves_unicode_outside_comments() {
        let input = "const bullets = \"• ─\";\nconst value = 1; // comment";
        let output = strip_javascript_noise(input, JavaScriptFlavor::Jsx);
        assert_eq!(output, "const bullets = \"• ─\";\nconst value = 1; //        ");
    }
}
