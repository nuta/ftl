use core::iter::Enumerate;
use core::iter::Peekable;
use core::slice::Iter;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnclosedQuote,
}

#[derive(Clone, Copy)]
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

pub struct Parameter<'a> {
    pub key: &'a [u8],
    pub value: &'a [u8],
}

pub struct Parser<'a> {
    input: &'a [u8],
    iter: Peekable<Enumerate<Iter<'a, u8>>>,
    state: State<'a>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        let iter = input.iter().enumerate().peekable();
        Self {
            input,
            iter,
            state: State::SkipWhitespace,
        }
    }
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Parameter<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((i, ch)) = self.iter.next() {
            match self.state {
                State::SkipWhitespace if ch.is_ascii_whitespace() => {
                    // Skip whitespace.
                }
                State::SkipWhitespace => {
                    // Non-whitespace character. This is the start of a key.
                    self.state = State::Key { start: i };
                }
                State::Key { start } if *ch == b'=' => {
                    let (quoted, value_start) = match self.iter.peek() {
                        Some((_, b'"')) => {
                            self.iter.next(); // consume the quote
                            (true, i + 2) // skip '=' and the quote
                        }
                        _ => (false, i + 1), // skip '='
                    };

                    self.state = State::Value {
                        key: &self.input[start..i],
                        start: value_start,
                        quoted,
                    };
                }
                State::Key { start } if ch.is_ascii_whitespace() => {
                    // A parameter without value (e.g. "debug").
                    self.state = State::SkipWhitespace;
                    return Some(Ok(Parameter {
                        key: &self.input[start..i],
                        value: &b""[..],
                    }));
                }
                State::Key { .. } => {
                    // A character in the key.
                }
                State::Value {
                    key: k,
                    start,
                    quoted,
                } if (quoted && *ch == b'"') || (!quoted && ch.is_ascii_whitespace()) => {
                    self.state = State::SkipWhitespace;
                    return Some(Ok(Parameter {
                        key: k,
                        value: &self.input[start..i],
                    }));
                }
                State::Value { .. } => {
                    // A character in the value.
                }
            }
        }

        match self.state {
            State::Key { start, .. } => {
                self.state = State::SkipWhitespace;
                return Some(Ok(Parameter {
                    key: &self.input[start..],
                    value: &b""[..],
                }));
            }
            State::Value { quoted: true, .. } => {
                self.state = State::SkipWhitespace;
                return Some(Err(Error::UnclosedQuote));
            }
            State::Value {
                key: k,
                start,
                quoted: false,
            } => {
                // A value at the end of the input.
                self.state = State::SkipWhitespace;
                return Some(Ok(Parameter {
                    key: k,
                    value: &self.input[start..],
                }));
            }
            State::SkipWhitespace => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse<'a>(input: &'a [u8], key: &[u8]) -> Result<Option<&'a [u8]>, Error> {
        for param in Parser::new(input) {
            let param = param?;
            if param.key == key {
                return Ok(Some(param.value));
            }
        }

        Ok(None)
    }

    #[test]
    fn test_parse() {
        let input = b"foo=123 bar=456";
        let result = parse(input, b"foo");
        assert_eq!(result, Ok(Some(&b"123"[..])));
    }

    #[test]
    fn test_parse_quoted_value() {
        let input = b"foo=\"hello world\" bar=456";
        assert_eq!(parse(input, b"foo"), Ok(Some(&b"hello world"[..])));
        assert_eq!(parse(input, b"bar"), Ok(Some(&b"456"[..])));
    }

    #[test]
    fn test_parse_empty_value() {
        let input = b"foo=123 bar=456 baz=";
        assert_eq!(parse(input, b"baz"), Ok(Some(&b""[..])));
    }

    #[test]
    fn test_parse_empty_input() {
        let input = b"";
        assert_eq!(parse(input, b"foo"), Ok(None));
    }

    #[test]
    fn test_parse_unclosed_quote() {
        let input = b"foo=123 bar=456 baz=\"";
        assert_eq!(parse(input, b"baz"), Err(Error::UnclosedQuote));
    }

    #[test]
    fn test_parse_bare_tokens() {
        let input = b"/ftl.elf ftl.lx.init=/bin/httpd nosmp debug ro";
        assert_eq!(parse(input, b"nosmp"), Ok(Some(&b""[..])));
        assert_eq!(parse(input, b"debug"), Ok(Some(&b""[..])));
        assert_eq!(parse(input, b"ro"), Ok(Some(&b""[..])));
        assert_eq!(parse(input, b"ftl.lx.init"), Ok(Some(&b"/bin/httpd"[..])));
    }
}
