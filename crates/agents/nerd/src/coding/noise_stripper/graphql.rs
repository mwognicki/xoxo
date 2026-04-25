//! GraphQL-specific noise stripping.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Code,
    String,
    BlockString,
    LineComment,
}

/// Strip comments from GraphQL source while preserving text shape.
///
/// Line count is preserved, `#` markers remain in place, and removed comment
/// content is replaced with spaces.
pub fn strip_graphql_noise(content: &str) -> String {
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
                    state = State::BlockString;
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
                        state = State::String;
                    }
                    _ => {
                        out.push(bytes[i] as char);
                        i += 1;
                    }
                }
            }
            State::String => {
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
            }
            State::BlockString => {
                if starts_with(bytes, i, b"\"\"\"") {
                    out.push_str("\"\"\"");
                    i += 3;
                    state = State::Code;
                    continue;
                }
                out.push(bytes[i] as char);
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

fn starts_with(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index + needle.len())
        .is_some_and(|candidate| candidate == needle)
}

#[cfg(test)]
mod tests {
    use super::strip_graphql_noise;

    #[test]
    fn strips_hash_comments() {
        let input = "type Query {\n  me: User # comment\n}\n";
        let output = strip_graphql_noise(input);
        assert_eq!(output, "type Query {\n  me: User #        \n}\n");
    }

    #[test]
    fn keeps_hash_inside_block_strings() {
        let input = "\"\"\"# not comment\nstill string\"\"\"\n";
        let output = strip_graphql_noise(input);
        assert_eq!(output, input);
    }
}
