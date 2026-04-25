//! Haskell-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

/// Strip comments from Haskell source while preserving text shape.
pub fn strip_haskell_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => {
                if starts_with(bytes, i, b"--") {
                    out.push('-');
                    out.push('-');
                    i += 2;
                    state = State::LineComment;
                    continue;
                }
                if starts_with(bytes, i, b"{-") {
                    out.push('{');
                    out.push('-');
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
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::SingleQuote => push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state),
            State::DoubleQuote => push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state),
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
                if starts_with(bytes, i, b"-}") {
                    out.push('-');
                    out.push('}');
                    i += 2;
                    state = State::Code;
                } else {
                    out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
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
    use super::strip_haskell_noise;

    #[test]
    fn strips_line_comments() {
        let input = "x = 1 -- comment\n";
        let output = strip_haskell_noise(input);
        assert_eq!(output, "x = 1 --        \n");
    }
}
