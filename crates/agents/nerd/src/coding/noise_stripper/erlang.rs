//! Erlang-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    LineComment,
}

/// Strip comments from Erlang source while preserving text shape.
pub fn strip_erlang_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => match bytes[i] {
                b'%' => {
                    out.push('%');
                    i += 1;
                    state = State::LineComment;
                }
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
            },
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

#[cfg(test)]
mod tests {
    use super::strip_erlang_noise;

    #[test]
    fn strips_percent_comments() {
        let input = "Value = 1. % comment\n";
        let output = strip_erlang_noise(input);
        assert_eq!(output, "Value = 1. %        \n");
    }
}
