#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnclosedQuote,
    KeyNotFound,
}

enum State<'a> {
    SkipWhitespace,
    Key {
        start: usize,
    },
    Value {
        key: &'a [u8],
        start: usize,
        quoted: bool,
    },
}

pub fn parse<'a>(input: &'a [u8], key: &[u8]) -> Result<&'a [u8], Error> {
    let mut iter = input.iter().enumerate().peekable();
    let mut state = State::SkipWhitespace;
    while let Some((i, ch)) = iter.next() {
        match state {
            State::SkipWhitespace if ch.is_ascii_whitespace() => {
                // Skip whitespace.
            }
            State::SkipWhitespace => {
                // Non-whitespace character. This is the start of a key.
                state = State::Key { start: i };
            }
            State::Key { start } if *ch == b'=' => {
                let (quoted, value_start) = match iter.peek() {
                    Some((_, b'"')) => {
                        iter.next(); // consume the quote
                        (true, i + 2) // skip '=' and the quote
                    }
                    _ => (false, i + 1), // skip '='
                };

                state = State::Value {
                    key: &input[start..i],
                    start: value_start,
                    quoted,
                };
            }
            State::Key { start } if ch.is_ascii_whitespace() => {
                // A parameter without value (e.g. "debug").
                if key != &input[start..i] {
                    // This is not the key we're looking for. Read the next key.
                    state = State::SkipWhitespace;
                    continue;
                }

                return Ok(&b""[..]);
            }
            State::Key { .. } => {
                // A character in the key.
            }
            State::Value {
                key: k,
                start,
                quoted,
            } if (quoted && *ch == b'"') || (!quoted && ch.is_ascii_whitespace()) => {
                if key != k {
                    // This is not the key we're looking for. Read the next key.
                    state = State::SkipWhitespace;
                    continue;
                }

                return Ok(&input[start..i]);
            }
            State::Value { .. } => {
                // A character in the value.
            }
        }
    }

    match state {
        State::Key { start, .. } => {
            if key != &input[start..] {
                // This is not the key we're looking for. Read the next key.
                return Err(Error::KeyNotFound);
            }

            return Ok(&b""[..]);
        }
        State::Value { quoted, .. } if quoted => {
            return Err(Error::UnclosedQuote);
        }
        State::Value {
            key: k,
            start,
            quoted,
        } if !quoted => {
            // A value at the end of the input.
            if key != k {
                // This is not the key we're looking for. Read the next key.
                return Err(Error::KeyNotFound);
            }

            return Ok(&input[start..]);
        }
        _ => Err(Error::KeyNotFound),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let input = b"foo=123 bar=456";
        let result = parse(input, b"foo");
        assert_eq!(result, Ok(&b"123"[..]));
    }

    #[test]
    fn test_parse_quoted_value() {
        let input = b"foo=\"hello world\" bar=456";
        assert_eq!(parse(input, b"foo"), Ok(&b"hello world"[..]));
        assert_eq!(parse(input, b"bar"), Ok(&b"456"[..]));
    }

    #[test]
    fn test_parse_empty_value() {
        let input = b"foo=123 bar=456 baz=";
        assert_eq!(parse(input, b"baz"), Ok(&b""[..]));
    }

    #[test]
    fn test_parse_empty_input() {
        let input = b"";
        assert_eq!(parse(input, b"foo"), Err(Error::KeyNotFound));
    }

    #[test]
    fn test_parse_unclosed_quote() {
        let input = b"foo=123 bar=456 baz=\"";
        assert_eq!(parse(input, b"baz"), Err(Error::UnclosedQuote));
    }

    #[test]
    fn test_parse_bare_tokens() {
        let input = b"/ftl.elf ftl.lx.init=/bin/httpd nosmp debug ro";
        assert_eq!(parse(input, b"nosmp"), Ok(&b""[..]));
        assert_eq!(parse(input, b"debug"), Ok(&b""[..]));
        assert_eq!(parse(input, b"ro"), Ok(&b""[..]));
        assert_eq!(parse(input, b"ftl.lx.init"), Ok(&b"/bin/httpd"[..]));
    }
}
