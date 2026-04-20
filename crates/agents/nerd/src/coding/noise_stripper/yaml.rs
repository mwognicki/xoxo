//! YAML-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Plain,
    SingleQuote,
    DoubleQuote,
}

/// Strip comments from YAML while preserving text shape.
///
/// This implementation is intentionally conservative. It strips `#` comments
/// only when they appear outside quoted strings and outside block scalar
/// content, and only when `#` is preceded by whitespace or is at line start.
pub fn strip_yaml_noise(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut block_scalar_indent: Option<usize> = None;

    for segment in content.split_inclusive('\n') {
        let (line, has_newline) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, true)
        } else {
            (segment, false)
        };

        let indent = leading_indent(line);
        if let Some(required_indent) = block_scalar_indent {
            if line.trim().is_empty() || indent >= required_indent {
                out.push_str(line);
                if has_newline {
                    out.push('\n');
                }
                continue;
            }
            block_scalar_indent = None;
        }

        let stripped_line = strip_yaml_line(line);
        let starts_block_scalar = detects_block_scalar(line);

        out.push_str(&stripped_line);
        if has_newline {
            out.push('\n');
        }

        if starts_block_scalar {
            block_scalar_indent = Some(indent + 1);
        }
    }

    out
}

fn strip_yaml_line(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut state = State::Plain;

    while i < bytes.len() {
        match state {
            State::Plain => match bytes[i] {
                b'#' if i == 0 || is_yaml_comment_prefix(bytes[i - 1]) => {
                    out.push('#');
                    i += 1;
                    while i < bytes.len() {
                        out.push(' ');
                        i += 1;
                    }
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
            State::SingleQuote => {
                out.push(bytes[i] as char);
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    state = State::Plain;
                }
                i += 1;
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
                    state = State::Plain;
                }
                i += 1;
            }
        }
    }

    out
}

fn leading_indent(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

fn detects_block_scalar(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut state = State::Plain;

    while i < bytes.len() {
        match state {
            State::Plain => match bytes[i] {
                b'#' if i == 0 || is_yaml_comment_prefix(bytes[i - 1]) => return false,
                b'\'' => {
                    i += 1;
                    state = State::SingleQuote;
                }
                b'"' => {
                    i += 1;
                    state = State::DoubleQuote;
                }
                b'|' | b'>' => {
                    let mut j = i + 1;
                    while let Some(byte) = bytes.get(j) {
                        if matches!(byte, b' ' | b'\t' | b'-' | b'+' | b'0'..=b'9') {
                            j += 1;
                            continue;
                        }
                        return *byte == b'#' && (j == 0 || is_yaml_comment_prefix(bytes[j - 1]));
                    }
                    return true;
                }
                _ => i += 1,
            },
            State::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    state = State::Plain;
                }
                i += 1;
            }
            State::DoubleQuote => {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    state = State::Plain;
                }
                i += 1;
            }
        }
    }

    false
}

fn is_yaml_comment_prefix(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

#[cfg(test)]
mod tests {
    use super::strip_yaml_noise;

    #[test]
    fn strips_line_comments() {
        let input = "name: demo # comment\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, "name: demo #        \n");
    }

    #[test]
    fn keeps_hash_inside_double_quoted_strings() {
        let input = "name: \"# not comment\"\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_hash_inside_single_quoted_strings() {
        let input = "name: '# not comment'\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn keeps_plain_scalar_hash_without_whitespace() {
        let input = "name: value#fragment\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn preserves_block_scalar_content() {
        let input = "body: |\n  # literal line\n  value\nnext: ok\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, input);
    }

    #[test]
    fn strips_comment_after_block_scalar_indicator() {
        let input = "body: | # comment\n  value\n";
        let output = strip_yaml_noise(input);
        assert_eq!(output, "body: | #        \n  value\n");
    }
}
