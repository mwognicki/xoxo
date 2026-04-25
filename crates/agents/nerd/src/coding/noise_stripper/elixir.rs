//! Elixir-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    TripleSingleQuote,
    TripleDoubleQuote,
    LineComment,
}

/// Strip comments from Elixir source while preserving text shape.
///
/// Line count is preserved, `#` markers remain in place, and removed comment
/// content is replaced with spaces.
pub fn strip_elixir_noise(content: &str) -> String {
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

                match bytes[i] {
                    b'#' => {
                        out.push('#');
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
                }
            }
            State::SingleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
            }
            State::DoubleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
            }
            State::TripleSingleQuote => {
                if starts_with(bytes, i, b"'''") {
                    out.push_str("'''");
                    i += 3;
                    state = State::Code;
                    continue;
                }
                push_triple_content(bytes, &mut out, &mut i);
            }
            State::TripleDoubleQuote => {
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"\"");
                    i += 3;
                    state = State::Code;
                    continue;
                }
                push_triple_content(bytes, &mut out, &mut i);
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

fn push_triple_content(bytes: &[u8], out: &mut String, index: &mut usize) {
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
    use super::strip_elixir_noise;

    #[test]
    fn strips_hash_comments() {
        let input = "value = 1 # comment\nIO.puts(value)\n";
        let output = strip_elixir_noise(input);
        assert_eq!(output, "value = 1 #        \nIO.puts(value)\n");
    }

    #[test]
    fn keeps_hash_inside_heredocs() {
        let input = "\"\"\"# not comment\nstill string\"\"\"\n";
        let output = strip_elixir_noise(input);
        assert_eq!(output, input);
    }
}
