//! C#-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    VerbatimString,
    LineComment,
    BlockComment,
}

/// Strip comments from C# source while preserving text shape.
///
/// Line count is preserved, comment delimiters remain in place, and removed
/// comment content is replaced with spaces.
pub fn strip_c_sharp_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => {
                if starts_with(bytes, i, b"//") {
                    out.push('/');
                    out.push('/');
                    i += 2;
                    state = State::LineComment;
                    continue;
                }

                if starts_with(bytes, i, b"/*") {
                    out.push('/');
                    out.push('*');
                    i += 2;
                    state = State::BlockComment;
                    continue;
                }

                if starts_with(bytes, i, b"@\"") {
                    out.push('@');
                    out.push('"');
                    i += 2;
                    state = State::VerbatimString;
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
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::SingleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
            }
            State::DoubleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
            }
            State::VerbatimString => {
                out.push(bytes[i] as char);
                if bytes[i] == b'"' {
                    if bytes.get(i + 1) == Some(&b'"') {
                        out.push('"');
                        i += 2;
                        continue;
                    }
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

#[cfg(test)]
mod tests {
    use super::strip_c_sharp_noise;

    #[test]
    fn strips_line_comments() {
        let input = "var value = 1; // comment\n";
        let output = strip_c_sharp_noise(input);
        assert_eq!(output, "var value = 1; //        \n");
    }

    #[test]
    fn keeps_comment_markers_inside_verbatim_strings() {
        let input = "var value = @\"// not comment /* nope */\";\n";
        let output = strip_c_sharp_noise(input);
        assert_eq!(output, input);
    }
}
