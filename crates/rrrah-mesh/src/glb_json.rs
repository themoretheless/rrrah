//! Bounded native JSON tape for glTF. All tokens borrow spans in immutable input.
use crate::{DecodeError, DecodeRequest, glb::GlbError};
use rrrah_core::{MemoryBudget, PixelBuffer};

pub const MAX_JSON_TOKENS: usize = 1_048_576;
const MAX_DEPTH: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonKind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}
#[derive(Debug, Clone, Copy)]
struct Token {
    kind: JsonKind,
    start: u32,
    end: u32,
    next: u32,
}
const EMPTY: Token = Token {
    kind: JsonKind::Null,
    start: 0,
    end: 0,
    next: 0,
};
#[derive(Debug, Clone)]
pub(crate) struct JsonTape {
    tokens: PixelBuffer<Token>,
}
impl JsonTape {
    pub(crate) fn parse(bytes: &[u8], request: &DecodeRequest) -> Result<Self, GlbError> {
        request.check_cancelled()?;
        if bytes.len() > crate::glb::MAX_GLB_JSON_BYTES {
            return Err(GlbError::Limit("JSON bytes"));
        }
        std::str::from_utf8(bytes).map_err(|_| GlbError::Invalid("JSON UTF-8"))?;
        let count = Parser::new(bytes, request, None).document()?;
        let fallback = MemoryBudget::new((MAX_JSON_TOKENS * size_of::<Token>()) as u64);
        let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
        let mut tokens = budget.try_buffer(count, EMPTY).map_err(DecodeError::Memory)?;
        Parser::new(bytes, request, Some(&mut tokens)).document()?;
        let tape = Self {
            tokens: tokens.freeze().into(),
        };
        Ok(tape)
    }
    pub(crate) fn root<'a>(&'a self, bytes: &'a [u8]) -> JsonValue<'a> {
        JsonValue {
            bytes,
            tokens: &self.tokens,
            index: 0,
        }
    }
}

/// A borrowed value; neither navigation nor scalar conversion allocates.
#[derive(Debug, Clone, Copy)]
pub struct JsonValue<'a> {
    bytes: &'a [u8],
    tokens: &'a [Token],
    index: usize,
}
impl<'a> JsonValue<'a> {
    pub fn kind(self) -> JsonKind {
        self.tokens[self.index].kind
    }
    pub fn raw_json(self) -> &'a str {
        let t = self.tokens[self.index];
        // The complete input and every string/number boundary were validated.
        std::str::from_utf8(&self.bytes[t.start as usize..t.end as usize]).unwrap()
    }
    pub fn string_eq(self, expected: &str) -> bool {
        self.kind() == JsonKind::String
            && StringChars::new(
                &self.bytes
                    [self.tokens[self.index].start as usize + 1..self.tokens[self.index].end as usize - 1],
            )
            .eq(expected.chars())
    }
    /// Decoded Unicode scalar stream over the original source string.
    pub fn string_chars(self) -> Option<impl Iterator<Item = char> + 'a> {
        (self.kind() == JsonKind::String).then(|| {
            let token = self.tokens[self.index];
            StringChars::new(&self.bytes[token.start as usize + 1..token.end as usize - 1])
        })
    }
    pub fn object(self) -> Option<JsonObject<'a>> {
        (self.kind() == JsonKind::Object).then_some(JsonObject {
            value: self,
            next: self.index + 1,
        })
    }
    /// Exact mathematical unsigned integer, including decimal/exponent notation.
    /// Never passes through f64 (which would lose large integer indices).
    pub fn as_u64(self) -> Option<u64> {
        if self.kind() != JsonKind::Number {
            return None;
        }
        exact_u64(self.raw_json().as_bytes())
    }
    pub fn as_f64(self) -> Option<f64> {
        if self.kind() != JsonKind::Number {
            return None;
        }
        let value: f64 = self.raw_json().parse().ok()?;
        value.is_finite().then_some(value)
    }
    pub fn as_bool(self) -> Option<bool> {
        (self.kind() == JsonKind::Bool).then(|| self.raw_json() == "true")
    }
    pub fn field(self, key: &str, request: &DecodeRequest) -> Result<Option<Self>, GlbError> {
        if self.kind() != JsonKind::Object {
            return Err(GlbError::Invalid("expected JSON object"));
        }
        let mut index = self.index + 1;
        let mut found = None;
        while index < self.tokens[self.index].next as usize {
            request.check_cancelled()?;
            let name = Self { index, ..self };
            let value = Self {
                index: index + 1,
                ..self
            };
            if name.string_eq(key) {
                found = Some(value);
            }
            index = self.tokens[index + 1].next as usize;
        }
        Ok(found)
    }
    pub fn array(self) -> Option<JsonArray<'a>> {
        (self.kind() == JsonKind::Array).then_some(JsonArray {
            value: self,
            next: self.index + 1,
        })
    }
}
#[derive(Debug)]
pub struct JsonArray<'a> {
    value: JsonValue<'a>,
    next: usize,
}
impl<'a> Iterator for JsonArray<'a> {
    type Item = JsonValue<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.value.tokens[self.value.index].next as usize {
            return None;
        }
        let value = JsonValue {
            index: self.next,
            ..self.value
        };
        self.next = self.value.tokens[self.next].next as usize;
        Some(value)
    }
}
fn exact_u64(bytes: &[u8]) -> Option<u64> {
    let negative = bytes.first() == Some(&b'-');
    let bytes = if negative { &bytes[1..] } else { bytes };
    let split = bytes
        .iter()
        .position(|b| matches!(b, b'e' | b'E'))
        .unwrap_or(bytes.len());
    let mantissa = &bytes[..split];
    let mut exponent = 0i64;
    if split < bytes.len() {
        let mut digits = &bytes[split + 1..];
        let sign = if digits.first() == Some(&b'-') { -1 } else { 1 };
        if digits.first().is_some_and(|b| matches!(b, b'-' | b'+')) {
            digits = &digits[1..];
        }
        for digit in digits {
            exponent = (exponent * 10 + i64::from(digit - b'0')).min(32_000_000);
        }
        exponent *= sign;
    }
    let fraction = mantissa
        .iter()
        .position(|b| *b == b'.')
        .map_or(0, |at| mantissa.len() - at - 1);
    let mut first = None;
    let mut last = 0;
    let mut digits = 0usize;
    for byte in mantissa {
        if *byte == b'.' {
            continue;
        }
        if *byte != b'0' {
            first.get_or_insert(digits);
            last = digits;
        }
        digits += 1;
    }
    let Some(first) = first else {
        return Some(0);
    };
    if negative {
        return None;
    }
    let power = exponent - fraction as i64 + (digits - last - 1) as i64;
    if power < 0 || (last - first + 1) as i64 + power > 20 {
        return None;
    }
    let mut value = 0u64;
    for byte in mantissa
        .iter()
        .filter(|b| **b != b'.')
        .skip(first)
        .take(last - first + 1)
    {
        value = value.checked_mul(10)?.checked_add(u64::from(byte - b'0'))?;
    }
    for _ in 0..power {
        value = value.checked_mul(10)?;
    }
    Some(value)
}

#[derive(Debug)]
pub struct JsonObject<'a> {
    value: JsonValue<'a>,
    next: usize,
}
impl<'a> Iterator for JsonObject<'a> {
    type Item = (JsonValue<'a>, JsonValue<'a>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.value.tokens[self.value.index].next as usize {
            return None;
        }
        let key = JsonValue {
            index: self.next,
            ..self.value
        };
        let value = JsonValue {
            index: self.next + 1,
            ..self.value
        };
        self.next = self.value.tokens[self.next + 1].next as usize;
        Some((key, value))
    }
}

struct Parser<'a, 'b> {
    bytes: &'a [u8],
    request: &'a DecodeRequest,
    at: usize,
    count: usize,
    next_cancel: usize,
    output: Option<&'b mut [Token]>,
}
impl<'a, 'b> Parser<'a, 'b> {
    fn new(bytes: &'a [u8], request: &'a DecodeRequest, output: Option<&'b mut [Token]>) -> Self {
        Self {
            bytes,
            request,
            at: if bytes.starts_with(b"\xef\xbb\xbf") { 3 } else { 0 },
            count: 0,
            next_cancel: 0,
            output,
        }
    }
    fn document(mut self) -> Result<usize, GlbError> {
        self.value(0)?;
        self.space()?;
        if self.at != self.bytes.len() {
            return Err(GlbError::Invalid("trailing JSON data"));
        }
        self.request.check_cancelled()?;
        Ok(self.count)
    }
    fn tick(&mut self) -> Result<(), GlbError> {
        if self.at >= self.next_cancel {
            self.request.check_cancelled()?;
            self.next_cancel = self.at.saturating_add(4096);
        }
        Ok(())
    }
    fn space(&mut self) -> Result<(), GlbError> {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.tick()?;
            self.at += 1;
        }
        Ok(())
    }
    fn byte(&mut self, expected: u8) -> Result<(), GlbError> {
        if self.bytes.get(self.at) != Some(&expected) {
            return Err(GlbError::Invalid("JSON punctuation"));
        }
        self.at += 1;
        Ok(())
    }
    fn add(&mut self) -> Result<usize, GlbError> {
        self.request.check_cancelled()?;
        if self.count == MAX_JSON_TOKENS {
            return Err(GlbError::Limit("JSON token count"));
        }
        let index = self.count;
        self.count += 1;
        Ok(index)
    }
    fn store(&mut self, index: usize, kind: JsonKind, start: usize) {
        if let Some(output) = &mut self.output {
            output[index] = Token {
                kind,
                start: start as u32,
                end: self.at as u32,
                next: self.count as u32,
            };
        }
    }
    fn string(&mut self) -> Result<(), GlbError> {
        self.byte(b'"')?;
        loop {
            self.tick()?;
            let byte = *self
                .bytes
                .get(self.at)
                .ok_or(GlbError::Invalid("unterminated JSON string"))?;
            self.at += 1;
            match byte {
                b'"' => return Ok(()),
                0..=31 => return Err(GlbError::Invalid("JSON string control byte")),
                b'\\' => {
                    let escaped = *self.bytes.get(self.at).ok_or(GlbError::Invalid("JSON escape"))?;
                    self.at += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        b'u' => {
                            let code = self.hex()?;
                            if (0xd800..=0xdbff).contains(&code) {
                                self.byte(b'\\')?;
                                self.byte(b'u')?;
                                if !(0xdc00..=0xdfff).contains(&self.hex()?) {
                                    return Err(GlbError::Invalid("JSON surrogate pair"));
                                }
                            } else if (0xdc00..=0xdfff).contains(&code) {
                                return Err(GlbError::Invalid("JSON isolated surrogate"));
                            }
                        }
                        _ => return Err(GlbError::Invalid("JSON escape")),
                    }
                }
                _ => {}
            }
        }
    }
    fn hex(&mut self) -> Result<u32, GlbError> {
        let mut value = 0;
        for _ in 0..4 {
            let byte = *self
                .bytes
                .get(self.at)
                .ok_or(GlbError::Invalid("JSON unicode escape"))?;
            let digit = (byte as char)
                .to_digit(16)
                .ok_or(GlbError::Invalid("JSON unicode escape"))?;
            value = value * 16 + digit;
            self.at += 1;
        }
        Ok(value)
    }
    fn digits(&mut self) -> Result<(), GlbError> {
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.tick()?;
            self.at += 1;
        }
        if start == self.at {
            return Err(GlbError::Invalid("JSON number digits"));
        }
        Ok(())
    }
    fn number(&mut self) -> Result<(), GlbError> {
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        if self.bytes.get(self.at) == Some(&b'0') {
            self.at += 1;
        } else {
            self.digits()?;
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            self.digits()?;
        }
        if self.bytes.get(self.at).is_some_and(|b| matches!(b, b'e' | b'E')) {
            self.at += 1;
            if self.bytes.get(self.at).is_some_and(|b| matches!(b, b'+' | b'-')) {
                self.at += 1;
            }
            self.digits()?;
        }
        Ok(())
    }
    fn value(&mut self, depth: usize) -> Result<(), GlbError> {
        self.space()?;
        if depth > MAX_DEPTH {
            return Err(GlbError::Limit("JSON depth"));
        }
        let start = self.at;
        let index = self.add()?;
        let kind = match self.bytes.get(self.at).copied() {
            Some(b'{') | Some(b'[') => {
                let object = self.bytes[self.at] == b'{';
                self.at += 1;
                let end = if object { b'}' } else { b']' };
                self.space()?;
                if self.bytes.get(self.at) != Some(&end) {
                    loop {
                        if object {
                            self.space()?;
                            let start = self.at;
                            let key = self.add()?;
                            self.string()?;
                            self.store(key, JsonKind::String, start);
                            self.space()?;
                            self.byte(b':')?;
                        }
                        self.value(depth + 1)?;
                        self.space()?;
                        if self.bytes.get(self.at) != Some(&b',') {
                            break;
                        }
                        self.at += 1;
                    }
                }
                self.byte(end)?;
                if object { JsonKind::Object } else { JsonKind::Array }
            }
            Some(b'"') => {
                self.string()?;
                JsonKind::String
            }
            Some(b'-' | b'0'..=b'9') => {
                self.number()?;
                JsonKind::Number
            }
            Some(b't' | b'f' | b'n') => {
                let (text, kind): (&[u8], _) = match self.bytes[self.at] {
                    b't' => (b"true", JsonKind::Bool),
                    b'f' => (b"false", JsonKind::Bool),
                    _ => (b"null", JsonKind::Null),
                };
                if !self.bytes[self.at..].starts_with(text) {
                    return Err(GlbError::Invalid("JSON literal"));
                }
                self.at += text.len();
                kind
            }
            _ => return Err(GlbError::Invalid("JSON value")),
        };
        self.store(index, kind, start);
        Ok(())
    }
}
struct StringChars<'a> {
    chars: std::str::Chars<'a>,
}
impl<'a> StringChars<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            chars: std::str::from_utf8(bytes).unwrap().chars(),
        }
    }
    fn hex(&mut self) -> u32 {
        (0..4).fold(0, |value, _| {
            value * 16 + self.chars.next().unwrap().to_digit(16).unwrap()
        })
    }
}
impl Iterator for StringChars<'_> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        let next = self.chars.next()?;
        if next != '\\' {
            return Some(next);
        }
        Some(match self.chars.next().unwrap() {
            '"' => '"',
            '\\' => '\\',
            '/' => '/',
            'b' => '\u{8}',
            'f' => '\u{c}',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'u' => {
                let mut code = self.hex();
                if (0xd800..=0xdbff).contains(&code) {
                    self.chars.next();
                    self.chars.next();
                    code = 0x10000 + (code - 0xd800) * 1024 + self.hex() - 0xdc00;
                }
                char::from_u32(code).unwrap()
            }
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> DecodeRequest {
        DecodeRequest::new("unused.glb")
    }
    #[test]
    fn independent_python_json_corpus_agrees_on_syntax_and_token_census() {
        let corpus = include_str!("../../../tests/fixtures/glb/json-corpus.tsv");
        for (index, line) in corpus.lines().enumerate() {
            let mut fields = line.split('\t');
            let valid = fields.next().unwrap() == "1";
            let count: usize = fields.next().unwrap().parse().unwrap();
            let hex = fields.next().unwrap();
            let data: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            let result = JsonTape::parse(&data, &request());
            assert_eq!(result.is_ok(), valid, "case {index}: {data:?}; {result:?}");
            if let Ok(tape) = result {
                assert_eq!(tape.tokens.len(), count, "case {index}");
            }
        }
    }
    #[test]
    fn escaped_names_last_duplicate_value_and_nested_navigation_preserve_source() {
        let bytes = br#"{"a":1,"\u0061":[{"\ud83d\ude00":"\u043a\u0443\u0431"},true,null,-2.5e1],"tail":7}"#;
        let request = request();
        let tape = JsonTape::parse(bytes, &request).unwrap();
        let mut array = tape
            .root(bytes)
            .field("a", &request)
            .unwrap()
            .unwrap()
            .array()
            .unwrap();
        assert!(
            array
                .next()
                .unwrap()
                .field("😀", &request)
                .unwrap()
                .unwrap()
                .string_eq("куб")
        );
        assert_eq!(array.next().unwrap().as_bool(), Some(true));
        assert_eq!(array.next().unwrap().kind(), JsonKind::Null);
        assert_eq!(array.next().unwrap().as_f64(), Some(-25.));
        assert!(array.next().is_none());
        assert_eq!(
            tape.root(bytes)
                .field("tail", &request)
                .unwrap()
                .unwrap()
                .as_u64(),
            Some(7)
        );
        assert_eq!(tape.root(bytes).raw_json(), std::str::from_utf8(bytes).unwrap());
    }
    #[test]
    fn independent_decimal_integer_oracle_agrees_without_f64_rounding() {
        for line in include_str!("../../../tests/fixtures/glb/json-integers.tsv").lines() {
            let (text, expected) = line.split_once('\t').unwrap();
            let expected = if expected == "-" {
                None
            } else {
                Some(expected.parse::<u64>().unwrap())
            };
            let tape = JsonTape::parse(text.as_bytes(), &request()).unwrap();
            assert_eq!(tape.root(text.as_bytes()).as_u64(), expected, "{text}");
        }
    }

    #[test]
    fn integer_notation_is_exact_beyond_f64_precision_and_at_u64_boundaries() {
        for (text, expected) in [
            ("100", Some(100)),
            ("100.0", Some(100)),
            ("1e2", Some(100)),
            ("1.000e+2", Some(100)),
            ("-0.0e999999999", Some(0)),
            ("1e-1", None),
            ("9007199254740993.0", Some(9007199254740993)),
            ("18446744073709551615.000", Some(u64::MAX)),
            ("184467440737095516150e-1", Some(u64::MAX)),
            ("18446744073709551616", None),
            ("1e999999999999999999999", None),
            ("0e-999999999999999999999", Some(0)),
            ("0.00100e3", Some(1)),
            ("-1", None),
        ] {
            let tape = JsonTape::parse(text.as_bytes(), &request()).unwrap();
            assert_eq!(tape.root(text.as_bytes()).as_u64(), expected, "{text}");
        }
        let tape = JsonTape::parse(b"1e99999", &request()).unwrap();
        assert_eq!(tape.root(b"1e99999").as_f64(), None);
    }
    #[test]
    fn exact_admission_last_owner_depth_token_limits_and_cancel() {
        let data = br#"{"array":[1,2,3]}"#;
        let bytes = 6 * size_of::<Token>() as u64;
        let budget = MemoryBudget::new(bytes);
        let mut request = request();
        request.memory_budget = Some(budget.clone());
        let tape = JsonTape::parse(data, &request).unwrap();
        assert_eq!(tape.tokens.len(), 6);
        assert_eq!(budget.used(), bytes);
        let clone = tape.clone();
        drop(tape);
        assert_eq!(budget.used(), bytes);
        drop(clone);
        assert_eq!(budget.used(), 0);
        request.memory_budget = Some(MemoryBudget::new(bytes - 1));
        assert!(matches!(
            JsonTape::parse(data, &request),
            Err(GlbError::Source(DecodeError::Memory(_)))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().peak(), 0);
        let too_deep = format!("{}0{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert!(matches!(
            JsonTape::parse(too_deep.as_bytes(), &request),
            Err(GlbError::Limit("JSON depth"))
        ));
        let too_many = format!("[{}0]", "0,".repeat(MAX_JSON_TOKENS));
        assert!(matches!(
            JsonTape::parse(too_many.as_bytes(), &request),
            Err(GlbError::Limit("JSON token count"))
        ));
        assert_eq!(request.memory_budget.as_ref().unwrap().used(), 0);
        request.cancellation = Some(crate::GenerationToken::new(
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(2)),
            1,
        ));
        assert!(matches!(
            JsonTape::parse(b"{}", &request),
            Err(GlbError::Source(DecodeError::Cancelled))
        ));
    }
}
