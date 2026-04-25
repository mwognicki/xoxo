//! Dart-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    TripleSingleQuote,
    TripleDoubleQuote,
    RawSingleQuote,
    RawDoubleQuote,
    LineComment,
    BlockComment,
}

/// Strip comments from Dart source while preserving text shape.
pub fn strip_dart_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => {
                if starts_with(bytes, i, b"'''") {
                    out.push_str("'''");
                    i += 3;
                    state = State::TripleSingleQuote;
                    continue;
                }
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"\"");
                    i += 3;
                    state = State::TripleDoubleQuote;
                    continue;
                }
                if starts_with(bytes, i, b"r'") {
                    out.push_str("r'");
                    i += 2;
                    state = State::RawSingleQuote;
                    continue;
                }
                if starts_with(bytes, i, b"r\"") {
                    out.push_str("r\"");
                    i += 2;
                    state = State::RawDoubleQuote;
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
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::SingleQuote => push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state),
            State::DoubleQuote => push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state),
            State::TripleSingleQuote => push_until(bytes, &mut out, &mut i, b"'''", State::Code, &mut state),
            State::TripleDoubleQuote => push_until(bytes, &mut out, &mut i, b"\"\"\"", State::Code, &mut state),
            State::RawSingleQuote => push_raw_until(bytes, &mut out, &mut i, b"'", State::Code, &mut state),
            State::RawDoubleQuote => push_raw_until(bytes, &mut out, &mut i, b"\"", State::Code, &mut state),
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

fn push_until(
    bytes: &[u8],
    out: &mut String,
    index: &mut usize,
    terminator: &[u8],
    next_state: State,
    state: &mut State,
) {
    if starts_with(bytes, *index, terminator) {
        out.push_str(std::str::from_utf8(terminator).expect("ascii terminator"));
        *index += terminator.len();
        *state = next_state;
        return;
    }
    out.push(bytes[*index] as char);
    if bytes[*index] == b'\\' {
        *index += 1;
        if *index < bytes.len() {
            out.push(bytes[*index] as char);
            *index += 1;
            return;
        }
    }
    *index += 1;
}

fn push_raw_until(
    bytes: &[u8],
    out: &mut String,
    index: &mut usize,
    terminator: &[u8],
    next_state: State,
    state: &mut State,
) {
    if starts_with(bytes, *index, terminator) {
        out.push_str(std::str::from_utf8(terminator).expect("ascii terminator"));
        *index += terminator.len();
        *state = next_state;
        return;
    }
    out.push(bytes[*index] as char);
    *index += 1;
}

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate == needle)
}

#[cfg(test)]
mod tests {
    use super::strip_dart_noise;

    #[test]
    fn strips_line_comments() {
        let input = "final value = 1; // comment\n";
        let output = strip_dart_noise(input);
        assert_eq!(output, "final value = 1; //        \n");
    }

    #[test]
    fn keeps_comment_markers_inside_raw_strings() {
        let input = "final text = r\"// not comment /* nope */\";\n";
        let output = strip_dart_noise(input);
        assert_eq!(output, input);
    }
}
