//! Go-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    DoubleQuote,
    RawString,
    Rune,
    LineComment { preserve: bool },
    BlockComment,
}

/// Strip comments from Go source while preserving text shape.
///
/// Line count is preserved, comment delimiters remain in place, and removed
/// comment content is replaced with spaces.
pub fn strip_go_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut line_start = 0usize;

    while i < bytes.len() {
        match state {
            State::Code => {
                if starts_with(bytes, i, b"//") {
                    let preserve = should_preserve_go_line_comment(bytes, i, line_start);
                    out.push('/');
                    out.push('/');
                    i += 2;
                    state = State::LineComment { preserve };
                    continue;
                }

                if starts_with(bytes, i, b"/*") {
                    out.push('/');
                    out.push('*');
                    i += 2;
                    state = State::BlockComment;
                    continue;
                }

                match bytes[i] {
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::DoubleQuote;
                    }
                    b'`' => {
                        out.push('`');
                        i += 1;
                        state = State::RawString;
                    }
                    b'\'' => {
                        out.push('\'');
                        i += 1;
                        state = State::Rune;
                    }
                    b'\n' => {
                        out.push('\n');
                        i += 1;
                        line_start = i;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::DoubleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
            }
            State::RawString => {
                out.push(bytes[i] as char);
                if bytes[i] == b'`' {
                    state = State::Code;
                }
                if bytes[i] == b'\n' {
                    line_start = i + 1;
                }
                i += 1;
            }
            State::Rune => {
                push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
            }
            State::LineComment { preserve } => {
                if bytes[i] == b'\n' {
                    out.push('\n');
                    i += 1;
                    state = State::Code;
                    line_start = i;
                } else if preserve {
                    out.push(bytes[i] as char);
                    i += 1;
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
                    line_start = i + 1;
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
    bytes: &[u8],
    out: &mut String,
    index: &mut usize,
    terminator: u8,
    next_state: State,
    state: &mut State,
) {
    out.push(bytes[*index] as char);
    if bytes[*index] == b'\\' {
        *index += 1;
        if *index < bytes.len() {
            out.push(bytes[*index] as char);
            *index += 1;
        }
        return;
    }

    if bytes[*index] == terminator {
        *state = next_state;
    }
    *index += 1;
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate == needle)
}

fn should_preserve_go_line_comment(bytes: &[u8], comment_start: usize, line_start: usize) -> bool {
    if comment_start + 2 > bytes.len() {
        return false;
    }

    let line_prefix = &bytes[line_start..comment_start];
    if !line_prefix
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
    {
        return false;
    }

    let Some(line_end_offset) = bytes[comment_start..].iter().position(|byte| *byte == b'\n') else {
        return is_go_directive_comment(&bytes[comment_start + 2..]);
    };

    is_go_directive_comment(&bytes[comment_start + 2..comment_start + line_end_offset])
}

fn is_go_directive_comment(comment_body: &[u8]) -> bool {
    let trimmed = trim_ascii_start(comment_body);

    trimmed.starts_with(b"go:")
        || trimmed.starts_with(b"+build")
        || trimmed.starts_with(b"line ")
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        .unwrap_or(bytes.len());
    &bytes[start..]
}

#[cfg(test)]
mod tests {
    use super::strip_go_noise;

    #[test]
    fn strips_line_comments() {
        let input = "value := 1 // comment\nfmt.Println(value)";
        let output = strip_go_noise(input);
        assert_eq!(output, "value := 1 //        \nfmt.Println(value)");
    }

    #[test]
    fn strips_block_comments() {
        let input = "var value = /* note */ 1";
        let output = strip_go_noise(input);
        assert_eq!(output, "var value = /*      */ 1");
    }

    #[test]
    fn keeps_comment_markers_inside_strings() {
        let input = "value := \"// not comment /* nope */\"";
        let output = strip_go_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_raw_strings() {
        let input = "value := `// not comment\n/* nope */`";
        let output = strip_go_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_runes() {
        let input = "slash := '/'\nstar := '*'";
        let output = strip_go_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_go_build_directives() {
        let input = "//go:build linux\n// +build linux\npackage main\n";
        let output = strip_go_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_line_directives() {
        let input = "//line generated.go:10\npackage main\n";
        let output = strip_go_noise(input);
        assert_eq!(output, input);
    }
}
