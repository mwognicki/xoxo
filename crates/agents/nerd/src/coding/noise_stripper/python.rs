//! Python-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    TripleSingleQuote,
    TripleDoubleQuote,
    LineComment,
}

/// Strip comments from Python source while preserving text shape.
///
/// Line count is preserved, `#` markers remain in place for stripped comments,
/// and removed comment content is replaced with spaces.
pub fn strip_python_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut at_line_start = true;

    while i < bytes.len() {
        match state {
            State::Code => {
                if bytes[i] == b'#' && !is_shebang_or_encoding_comment(bytes, i, at_line_start) {
                    out.push('#');
                    i += 1;
                    state = State::LineComment;
                    continue;
                }

                if starts_with(bytes, i, b"'''") {
                    out.push_str("'''");
                    i += 3;
                    state = State::TripleSingleQuote;
                    at_line_start = false;
                    continue;
                }

                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"\"");
                    i += 3;
                    state = State::TripleDoubleQuote;
                    at_line_start = false;
                    continue;
                }

                match bytes[i] {
                    b'\'' => {
                        out.push('\'');
                        i += 1;
                        state = State::SingleQuote;
                        at_line_start = false;
                    }
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::DoubleQuote;
                        at_line_start = false;
                    }
                    b'\n' => {
                        out.push('\n');
                        i += 1;
                        at_line_start = true;
                    }
                    b' ' | b'\t' | b'\r' => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                        at_line_start = false;
                    }
                }
            }
            State::SingleQuote => {
                push_python_string(bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
                at_line_start = false;
            }
            State::DoubleQuote => {
                push_python_string(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
                at_line_start = false;
            }
            State::TripleSingleQuote => {
                out.push(bytes[i] as char);
                if starts_with(bytes, i, b"'''") {
                    out.push_str("''");
                    i += 3;
                    state = State::Code;
                } else {
                    i += 1;
                }
                at_line_start = bytes.get(i.wrapping_sub(1)) == Some(&b'\n');
            }
            State::TripleDoubleQuote => {
                out.push(bytes[i] as char);
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"");
                    i += 3;
                    state = State::Code;
                } else {
                    i += 1;
                }
                at_line_start = bytes.get(i.wrapping_sub(1)) == Some(&b'\n');
            }
            State::LineComment => {
                if bytes[i] == b'\n' {
                    out.push('\n');
                    i += 1;
                    state = State::Code;
                    at_line_start = true;
                } else {
                    out.push(' ');
                    i += 1;
                }
            }
        }
    }

    out
}

fn push_python_string(
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

fn is_shebang_or_encoding_comment(bytes: &[u8], index: usize, at_line_start: bool) -> bool {
    if !at_line_start {
        return false;
    }

    if bytes.get(index + 1) == Some(&b'!') {
        return true;
    }

    let Some(line_end) = bytes[index..].iter().position(|byte| *byte == b'\n') else {
        return line_contains_encoding_marker(&bytes[index + 1..]);
    };

    line_contains_encoding_marker(&bytes[index + 1..index + line_end])
}

fn line_contains_encoding_marker(line: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(line) else {
        return false;
    };

    text.contains("coding=") || text.contains("coding:")
}

#[cfg(test)]
mod tests {
    use super::strip_python_noise;

    #[test]
    fn strips_line_comments() {
        let input = "value = 1  # comment\nprint(value)";
        let output = strip_python_noise(input);
        assert_eq!(output, "value = 1  #        \nprint(value)");
    }

    #[test]
    fn keeps_comment_markers_inside_strings() {
        let input = "value = \"# not comment\"";
        let output = strip_python_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_single_quoted_strings() {
        let input = "value = '# not comment'";
        let output = strip_python_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_comment_markers_inside_triple_quoted_strings() {
        let input = "value = \"\"\"# not comment\nstill string\"\"\"";
        let output = strip_python_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_shebang_lines() {
        let input = "#!/usr/bin/env python3\nprint('hi')\n";
        let output = strip_python_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_encoding_comments() {
        let input = "# -*- coding: utf-8 -*-\nvalue = 1\n";
        let output = strip_python_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn strips_inline_comment_after_string() {
        let input = "value = \"hi\"  # comment";
        let output = strip_python_noise(input);
        assert_eq!(output, "value = \"hi\"  #        ");
    }
}
