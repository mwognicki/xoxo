//! Fortran-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    SingleQuote,
    DoubleQuote,
    LineComment,
}

/// Strip comments from Fortran source while preserving text shape.
pub fn strip_fortran_noise(content: &str) -> String {
    let mut out = String::with_capacity(content.len());

    for segment in content.split_inclusive('\n') {
        let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, true)
        } else {
            (segment, false)
        };

        out.push_str(&strip_fortran_line(line));
        if has_newline {
            out.push('\n');
        }
    }

    out
}

fn strip_fortran_line(line: &str) -> String {
    if is_fixed_form_comment(line) {
        let mut out = String::with_capacity(line.len());
        if let Some(first) = line.chars().next() {
            out.push(first);
            for ch in line.chars().skip(1) {
                out.push(if ch == '\t' { '\t' } else { ' ' });
            }
        }
        return out;
    }

    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut state = State::Code;

    while i < bytes.len() {
        match state {
            State::Code => match bytes[i] {
                b'!' => {
                    out.push('!');
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
            State::SingleQuote => push_quote(bytes, &mut out, &mut i, b'\'', State::Code, &mut state),
            State::DoubleQuote => push_quote(bytes, &mut out, &mut i, b'"', State::Code, &mut state),
            State::LineComment => {
                out.push(' ');
                i += 1;
            }
        }
    }

    out
}

fn is_fixed_form_comment(line: &str) -> bool {
    matches!(line.as_bytes().first(), Some(b'c' | b'C' | b'*' | b'!'))
}

fn push_quote(
    bytes: &[u8],
    out: &mut String,
    index: &mut usize,
    terminator: u8,
    next_state: State,
    state: &mut State,
) {
    out.push(bytes[*index] as char);
    if bytes[*index] == terminator {
        if bytes.get(*index + 1) == Some(&terminator) {
            out.push(terminator as char);
            *index += 2;
            return;
        }
        *state = next_state;
    }
    *index += 1;
}

#[cfg(test)]
mod tests {
    use super::strip_fortran_noise;

    #[test]
    fn strips_inline_bang_comments() {
        let input = "print *, \"hi\" ! comment\n";
        let output = strip_fortran_noise(input);
        assert_eq!(output, "print *, \"hi\" !        \n");
    }

    #[test]
    fn strips_fixed_form_comments() {
        let input = "C comment line\n";
        let output = strip_fortran_noise(input);
        assert_eq!(output, "C             \n");
    }
}
