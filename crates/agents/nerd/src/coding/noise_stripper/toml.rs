//! TOML-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    BasicString,
    LiteralString,
    MultilineBasicString,
    MultilineLiteralString,
    LineComment,
}

/// Strip comments from TOML while preserving text shape.
///
/// Line count is preserved, `#` markers remain in place for stripped comments,
/// and removed comment content is replaced with spaces.
pub fn strip_toml_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => {
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"\"");
                    i += 3;
                    state = State::MultilineBasicString;
                    continue;
                }
                if starts_with(bytes, i, b"'''") {
                    out.push_str("'''");
                    i += 3;
                    state = State::MultilineLiteralString;
                    continue;
                }

                match bytes[i] {
                    b'#' => {
                        out.push('#');
                        i += 1;
                        state = State::LineComment;
                    }
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::BasicString;
                    }
                    b'\'' => {
                        out.push('\'');
                        i += 1;
                        state = State::LiteralString;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::BasicString => {
                push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
            }
            State::LiteralString => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\'' {
                    state = State::Code;
                }
                i += 1;
            }
            State::MultilineBasicString => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    continue;
                }
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push('"');
                    out.push('"');
                    i += 3;
                    state = State::Code;
                    continue;
                }
                i += 1;
            }
            State::MultilineLiteralString => {
                out.push(bytes[i] as char);
                if starts_with(bytes, i, b"'''") {
                    out.push('\'');
                    out.push('\'');
                    i += 3;
                    state = State::Code;
                    continue;
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
    use super::strip_toml_noise;

    #[test]
    fn strips_line_comments() {
        let input = "name = \"demo\" # comment\n";
        let output = strip_toml_noise(input);
        assert_eq!(output, "name = \"demo\" #        \n");
    }

    #[test]
    fn keeps_hash_inside_basic_strings() {
        let input = "value = \"# not comment\"\n";
        let output = strip_toml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_hash_inside_literal_strings() {
        let input = "value = '# not comment'\n";
        let output = strip_toml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_hash_inside_multiline_basic_strings() {
        let input = "value = \"\"\"# not comment\nstill string\"\"\"\n";
        let output = strip_toml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_hash_inside_multiline_literal_strings() {
        let input = "value = '''# not comment\nstill string'''\n";
        let output = strip_toml_noise(input);
        assert_eq!(output, input);
    }
}
