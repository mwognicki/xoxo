//! Shell-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    Backtick,
    LineComment,
}

/// Strip comments from shell source while preserving text shape.
///
/// Line count is preserved, `#` markers remain in place, and removed comment
/// content is replaced with spaces.
pub fn strip_shell_noise(content: &str) -> String {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut at_line_start = true;

    while i < bytes.len() {
        match state {
            State::Code => match bytes[i] {
                b'#' if !is_shebang(bytes, i, at_line_start) => {
                    out.push('#');
                    i += 1;
                    state = State::LineComment;
                }
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
                b'`' => {
                    out.push('`');
                    i += 1;
                    state = State::Backtick;
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
            },
            State::SingleQuote => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\'' {
                    state = State::Code;
                }
                i += 1;
                at_line_start = false;
            }
            State::DoubleQuote => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    continue;
                }
                if bytes[i] == b'"' {
                    state = State::Code;
                }
                i += 1;
                at_line_start = false;
            }
            State::Backtick => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\\' {
                    i += 1;
                    if i < bytes.len() {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    continue;
                }
                if bytes[i] == b'`' {
                    state = State::Code;
                }
                i += 1;
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
        }
    }

    out
}

fn is_shebang(bytes: &[u8], index: usize, at_line_start: bool) -> bool {
    at_line_start && bytes.get(index + 1) == Some(&b'!')
}

#[cfg(test)]
mod tests {
    use super::strip_shell_noise;

    #[test]
    fn strips_hash_comments() {
        let input = "echo hi # comment\n";
        let output = strip_shell_noise(input);
        assert_eq!(output, "echo hi #        \n");
    }

    #[test]
    fn preserves_shebang_lines() {
        let input = "#!/usr/bin/env bash\necho hi\n";
        let output = strip_shell_noise(input);
        assert_eq!(output, input);
    }
}
