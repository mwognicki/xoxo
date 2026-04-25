//! VB.NET-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    DoubleQuote,
    LineComment,
}

/// Strip comments from VB.NET source while preserving text shape.
pub fn strip_vb_net_noise(content: &str) -> String {
    let mut out = String::with_capacity(content.len());

    for segment in content.split_inclusive('\n') {
        let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, true)
        } else {
            (segment, false)
        };

        out.push_str(&strip_vb_net_line(line));
        if has_newline {
            out.push('\n');
        }
    }

    out
}

fn strip_vb_net_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut state = State::Code;
    let mut line_start = true;

    while i < bytes.len() {
        match state {
            State::Code => {
                if bytes[i] == b'\'' {
                    out.push('\'');
                    i += 1;
                    state = State::LineComment;
                    continue;
                }

                if line_start && starts_with_ignore_ascii_case(bytes, i, b"Rem") {
                    let next = bytes.get(i + 3);
                    if next.is_none_or(|b| matches!(b, b' ' | b'\t')) {
                        out.push_str("Rem");
                        i += 3;
                        state = State::LineComment;
                        continue;
                    }
                }

                match bytes[i] {
                    b'"' => {
                        out.push('"');
                        i += 1;
                        state = State::DoubleQuote;
                        line_start = false;
                    }
                    b' ' | b'\t' => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                        line_start = false;
                    }
                }
            }
            State::DoubleQuote => {
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
                out.push(' ');
                i += 1;
            }
        }
    }

    out
}

fn starts_with_ignore_ascii_case(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes.get(index..index + needle.len()).is_some_and(|candidate| {
        candidate.eq_ignore_ascii_case(needle)
    })
}

#[cfg(test)]
mod tests {
    use super::strip_vb_net_noise;

    #[test]
    fn strips_apostrophe_comments() {
        let input = "Dim x = 1 ' comment\n";
        let output = strip_vb_net_noise(input);
        assert_eq!(output, "Dim x = 1 '        \n");
    }
}
