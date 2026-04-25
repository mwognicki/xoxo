//! Ruby-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    LineComment,
    BlockComment,
}

/// Strip comments from Ruby source while preserving text shape.
///
/// Line count is preserved, comment delimiters remain in place, and removed
/// comment content is replaced with spaces.
pub fn strip_ruby_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut at_line_start = true;

    while i < bytes.len() {
        match state {
            State::Code => {
                if at_line_start && starts_with(bytes, i, b"=begin") {
                    out.push_str("=begin");
                    i += 6;
                    state = State::BlockComment;
                    at_line_start = false;
                    continue;
                }

                if bytes[i] == b'#' && !is_shebang(bytes, i, at_line_start) {
                    out.push('#');
                    i += 1;
                    state = State::LineComment;
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
                push_with_escape(bytes, &mut out, &mut i, b'\'', State::Code, &mut state);
                at_line_start = false;
            }
            State::DoubleQuote => {
                push_with_escape(bytes, &mut out, &mut i, b'"', State::Code, &mut state);
                at_line_start = false;
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
            State::BlockComment => {
                if bytes[i] == b'\n' {
                    out.push('\n');
                    i += 1;
                    at_line_start = true;
                    continue;
                }

                if at_line_start && starts_with(bytes, i, b"=end") {
                    out.push_str("=end");
                    i += 4;
                    state = State::Code;
                    at_line_start = false;
                    continue;
                }

                out.push(' ');
                i += 1;
                at_line_start = false;
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

fn is_shebang(bytes: &[u8], index: usize, at_line_start: bool) -> bool {
    at_line_start && bytes.get(index + 1) == Some(&b'!')
}

#[cfg(test)]
mod tests {
    use super::strip_ruby_noise;

    #[test]
    fn strips_line_comments() {
        let input = "value = 1 # comment\nputs value\n";
        let output = strip_ruby_noise(input);
        assert_eq!(output, "value = 1 #        \nputs value\n");
    }

    #[test]
    fn strips_begin_end_block_comments() {
        let input = "=begin\ncomment\n=end\nputs 1\n";
        let output = strip_ruby_noise(input);
        assert_eq!(output, "=begin\n       \n=end\nputs 1\n");
    }
}
