//! Borrowed PostScript lexical tokens. Interpretation and decoded strings are
//! separate stages; token payloads retain their original escapes and encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpsToken<'a> {
    Word(&'a [u8]),
    LiteralName(&'a [u8]),
    ImmediateName(&'a [u8]),
    String(&'a [u8]),
    HexString(&'a [u8]),
    Ascii85String(&'a [u8]),
    ArrayStart,
    ArrayEnd,
    ProcedureStart,
    ProcedureEnd,
    DictionaryStart,
    DictionaryEnd,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EpsTokenError {
    #[error("invalid PostScript token at byte {0}")]
    Syntax(usize),
    #[error("PostScript lexical resource limit exceeded")]
    Limit,
    #[error("PostScript tokenization cancelled")]
    Cancelled,
}
fn space(b: u8) -> bool {
    matches!(b, 0 | b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}
fn delimiter(b: u8) -> bool {
    space(b)
        || matches!(
            b,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}

pub struct EpsTokens<'a, F> {
    bytes: &'a [u8],
    position: usize,
    remaining: usize,
    max_token_bytes: usize,
    failed: bool,
    cancelled: F,
}
impl<F> std::fmt::Debug for EpsTokens<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EpsTokens")
            .field("position", &self.position)
            .field("remaining", &self.remaining)
            .field("max_token_bytes", &self.max_token_bytes)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}
impl<'a, F: FnMut() -> bool> EpsTokens<'a, F> {
    pub(crate) fn new(bytes: &'a [u8], max_tokens: usize, max_token_bytes: usize, cancelled: F) -> Self {
        Self {
            bytes,
            position: 0,
            remaining: max_tokens,
            max_token_bytes,
            failed: false,
            cancelled,
        }
    }
    fn advance(&mut self) -> Result<(), EpsTokenError> {
        if self.position % 4096 == 0 && (self.cancelled)() {
            return Err(EpsTokenError::Cancelled);
        }
        self.position += 1;
        Ok(())
    }
    fn scan(&mut self) -> Result<Option<EpsToken<'a>>, EpsTokenError> {
        use EpsTokenError as E;
        if (self.cancelled)() {
            return Err(E::Cancelled);
        }
        while let Some(&b) = self.bytes.get(self.position) {
            if space(b) {
                self.advance()?;
            } else if b == b'%' {
                while self
                    .bytes
                    .get(self.position)
                    .is_some_and(|b| !matches!(b, b'\r' | b'\n' | 0x0c))
                {
                    self.advance()?;
                }
            } else {
                break;
            }
        }
        let start = self.position;
        let Some(&b) = self.bytes.get(start) else {
            return Ok(None);
        };
        if self.remaining == 0 {
            return Err(E::Limit);
        }
        self.remaining -= 1;
        self.advance()?;
        let token = match b {
            b'[' => EpsToken::ArrayStart,
            b']' => EpsToken::ArrayEnd,
            b'{' => EpsToken::ProcedureStart,
            b'}' => EpsToken::ProcedureEnd,
            b')' => return Err(E::Syntax(start)),
            b'>' => {
                if self.bytes.get(self.position) != Some(&b'>') {
                    return Err(E::Syntax(start));
                }
                self.advance()?;
                EpsToken::DictionaryEnd
            }
            b'<' => {
                if self.bytes.get(self.position) == Some(&b'<') {
                    self.advance()?;
                    EpsToken::DictionaryStart
                } else {
                    let ascii85 = self.bytes.get(self.position) == Some(&b'~');
                    if ascii85 {
                        self.advance()?;
                    }
                    let content = self.position;
                    loop {
                        if self.position - start > self.max_token_bytes {
                            return Err(E::Limit);
                        }
                        let c = *self.bytes.get(self.position).ok_or(E::Syntax(start))?;
                        if ascii85 && c == b'~' {
                            let end = self.position;
                            self.advance()?;
                            if self.bytes.get(self.position) != Some(&b'>') {
                                return Err(E::Syntax(start));
                            }
                            self.advance()?;
                            break EpsToken::Ascii85String(&self.bytes[content..end]);
                        }
                        if !ascii85 && c == b'>' {
                            let end = self.position;
                            self.advance()?;
                            break EpsToken::HexString(&self.bytes[content..end]);
                        }
                        // ASCII85 group/value validity is checked when decoding the string.
                        if !space(c)
                            && !(if ascii85 {
                                (b'!'..=b'u').contains(&c) || c == b'z'
                            } else {
                                c.is_ascii_hexdigit()
                            })
                        {
                            return Err(E::Syntax(self.position));
                        }
                        self.advance()?;
                    }
                }
            }
            b'(' => {
                let content = self.position;
                let mut depth = 1usize;
                loop {
                    if self.position - start > self.max_token_bytes {
                        return Err(E::Limit);
                    }
                    let c = *self.bytes.get(self.position).ok_or(E::Syntax(start))?;
                    let end = self.position;
                    self.advance()?;
                    if c == b'\\' {
                        let escaped = *self.bytes.get(self.position).ok_or(E::Syntax(start))?;
                        self.advance()?;
                        if escaped == b'\r' && self.bytes.get(self.position) == Some(&b'\n') {
                            self.advance()?;
                        }
                    } else if c == b'(' {
                        depth += 1;
                        if depth > 128 {
                            return Err(E::Limit);
                        }
                    } else if c == b')' {
                        depth -= 1;
                        if depth == 0 {
                            break EpsToken::String(&self.bytes[content..end]);
                        }
                    }
                }
            }
            _ => {
                let name = b == b'/';
                let immediate = name && self.bytes.get(self.position) == Some(&b'/');
                if immediate {
                    self.advance()?;
                }
                let content = if name { self.position } else { start };
                while self.bytes.get(self.position).is_some_and(|b| !delimiter(*b)) {
                    if self.position - start >= self.max_token_bytes {
                        return Err(E::Limit);
                    }
                    self.advance()?;
                }
                let bytes = &self.bytes[content..self.position];
                if immediate {
                    EpsToken::ImmediateName(bytes)
                } else if name {
                    EpsToken::LiteralName(bytes)
                } else {
                    EpsToken::Word(bytes)
                }
            }
        };
        if self.position - start > self.max_token_bytes {
            return Err(E::Limit);
        }
        Ok(Some(token))
    }
}
impl<'a, F: FnMut() -> bool> Iterator for EpsTokens<'a, F> {
    type Item = Result<EpsToken<'a>, EpsTokenError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        match self.scan() {
            Ok(Some(t)) => Some(Ok(t)),
            Ok(None) => {
                self.failed = true;
                None
            }
            Err(e) => {
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexical_groups_strings_names_and_comments_preserve_source() {
        let source = b"% header\r\n/x //y [ -2 16#FF ] { dup } << /s (a(b)\\)c) /h <A f0> /a <~z!!~> >>";
        let tokens = EpsTokens::new(source, 32, 128, || false)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            tokens,
            [
                EpsToken::LiteralName(b"x"),
                EpsToken::ImmediateName(b"y"),
                EpsToken::ArrayStart,
                EpsToken::Word(b"-2"),
                EpsToken::Word(b"16#FF"),
                EpsToken::ArrayEnd,
                EpsToken::ProcedureStart,
                EpsToken::Word(b"dup"),
                EpsToken::ProcedureEnd,
                EpsToken::DictionaryStart,
                EpsToken::LiteralName(b"s"),
                EpsToken::String(b"a(b)\\)c"),
                EpsToken::LiteralName(b"h"),
                EpsToken::HexString(b"A f0"),
                EpsToken::LiteralName(b"a"),
                EpsToken::Ascii85String(b"z!!"),
                EpsToken::DictionaryEnd
            ]
        );
        let EpsToken::LiteralName(name) = tokens[0] else {
            panic!()
        };
        assert_eq!(name.as_ptr(), source[11..].as_ptr());
    }
    #[test]
    fn malformed_limits_and_one_shot_cancellation_stop_iteration() {
        for input in [
            b"(unterminated".as_slice(),
            b"<abQ>",
            b"<~bad~x",
            b">",
            b")",
            b"(a\\",
        ] {
            let mut tokens = EpsTokens::new(input, 32, 128, || false);
            assert!(tokens.next().unwrap().is_err());
            assert!(tokens.next().is_none());
        }
        let mut tokens = EpsTokens::new(b"abc def", 1, 3, || false);
        assert!(tokens.next().unwrap().is_ok());
        assert_eq!(tokens.next(), Some(Err(EpsTokenError::Limit)));
        assert_eq!(
            EpsTokens::new(b"(abcd)", 10, 4, || false).next(),
            Some(Err(EpsTokenError::Limit))
        );
        let input = vec![b' '; 16384];
        let mut polls = 0;
        let mut tokens = EpsTokens::new(&input, 10, 32, || {
            polls += 1;
            polls == 3
        });
        assert_eq!(tokens.next(), Some(Err(EpsTokenError::Cancelled)));
        assert!(tokens.next().is_none());
    }
}
