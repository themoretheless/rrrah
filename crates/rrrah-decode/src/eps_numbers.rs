//! Allocation-free PostScript number recognition, using 32-bit integer/real objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EpsNumber {
    Integer(i32),
    Real(f32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EpsNumberError {
    #[error("PostScript numeric token byte limit exceeded")]
    Limit,
    #[error("PostScript number exceeds the supported 32-bit numeric range")]
    Range,
    #[error("PostScript numeric recognition cancelled")]
    Cancelled,
}
fn digit(b: u8) -> Option<u32> {
    match b {
        b'0'..=b'9' => Some(u32::from(b - b'0')),
        b'a'..=b'z' => Some(u32::from(b - b'a') + 10),
        b'A'..=b'Z' => Some(u32::from(b - b'A') + 10),
        _ => None,
    }
}
/// Recognize a complete executable word as a PostScript number. `None` means
/// the word is a name, including malformed numeric-looking names. Decimal integer
/// overflow promotes to a real; radix values are unsigned 32-bit bit patterns
/// reinterpreted as signed integers. Non-finite reals and radix overflow refuse.
/// Source bytes remain borrowed, with no allocation. Scanning polls cancellation
/// for each byte; standard-library numeric conversion is bounded by the caller's
/// token byte limit and polls immediately before and after conversion.
pub fn parse_eps_number<F: FnMut() -> bool>(
    word: &[u8],
    max_word_bytes: usize,
    mut cancelled: F,
) -> Result<Option<EpsNumber>, EpsNumberError> {
    use EpsNumberError as E;
    if cancelled() {
        return Err(E::Cancelled);
    }
    if word.len() > max_word_bytes {
        return Err(E::Limit);
    }
    if word.is_empty() {
        return Ok(None);
    }
    let mut hash = None;
    for (i, &b) in word.iter().enumerate() {
        if cancelled() {
            return Err(E::Cancelled);
        }
        if b == b'#' {
            if hash.is_some() {
                return Ok(None);
            }
            hash = Some(i);
        }
    }
    if let Some(at) = hash {
        let (base_text, tail) = word.split_at(at);
        let digits = &tail[1..];
        if base_text.is_empty() || digits.is_empty() {
            return Ok(None);
        }
        let mut base = Some(0u32);
        for &b in base_text {
            if cancelled() {
                return Err(E::Cancelled);
            }
            if !b.is_ascii_digit() {
                return Ok(None);
            }
            base = base.and_then(|v| v.checked_mul(10)?.checked_add(u32::from(b - b'0')));
        }
        let Some(base @ 2..=36) = base else {
            return Ok(None);
        };
        let mut value = Some(0u32);
        for &b in digits {
            if cancelled() {
                return Err(E::Cancelled);
            }
            let Some(d) = digit(b).filter(|d| *d < base) else {
                return Ok(None);
            };
            value = value.and_then(|v| v.checked_mul(base)?.checked_add(d));
        }
        if cancelled() {
            return Err(E::Cancelled);
        }
        return value
            .map(|v| Some(EpsNumber::Integer(i32::from_ne_bytes(v.to_ne_bytes()))))
            .ok_or(E::Range);
    }
    // Full grammar validation prevents Rust-specific spellings such as inf/NaN
    // from becoming numeric objects, and never parses a prefix of a name.
    let mut i = usize::from(matches!(word[0], b'+' | b'-'));
    let mut digits = 0;
    while word.get(i).is_some_and(u8::is_ascii_digit) {
        if cancelled() {
            return Err(E::Cancelled);
        }
        i += 1;
        digits += 1;
    }
    let mut real = false;
    if word.get(i) == Some(&b'.') {
        real = true;
        i += 1;
        while word.get(i).is_some_and(u8::is_ascii_digit) {
            if cancelled() {
                return Err(E::Cancelled);
            }
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Ok(None);
    }
    if word.get(i).is_some_and(|b| matches!(b, b'e' | b'E')) {
        real = true;
        i += 1;
        if word.get(i).is_some_and(|b| matches!(b, b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while word.get(i).is_some_and(u8::is_ascii_digit) {
            if cancelled() {
                return Err(E::Cancelled);
            }
            i += 1;
        }
        if i == start {
            return Ok(None);
        }
    }
    if i != word.len() {
        return Ok(None);
    }
    if cancelled() {
        return Err(E::Cancelled);
    }
    // The grammar above admits only ASCII, so this conversion cannot fail.
    let text = std::str::from_utf8(word).map_err(|_| E::Range)?;
    let result = if !real {
        text.parse::<i32>().map(EpsNumber::Integer).ok()
    } else {
        None
    };
    let result = match result {
        Some(number) => number,
        None => {
            let value = text.parse::<f32>().map_err(|_| E::Range)?;
            if !value.is_finite() {
                return Err(E::Range);
            }
            EpsNumber::Real(value)
        }
    };
    if cancelled() {
        return Err(E::Cancelled);
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn postscript_decimal_and_radix_numbers_keep_numeric_object_types() {
        let integers: &[(&[u8], i32)] = &[
            (b"123", 123),
            (b"-98", -98),
            (b"+17", 17),
            (b"-0", 0),
            (b"2147483647", i32::MAX),
            (b"-2147483648", i32::MIN),
            (b"8#1777", 1023),
            (b"16#FFFE", 65534),
            (b"2#1000", 8),
            (b"16#FFFFFFFF", -1),
            (b"16#80000000", i32::MIN),
            (b"36#z", 35),
            (b"00016#fF", 255),
        ];
        for &(word, expected) in integers {
            assert_eq!(
                parse_eps_number(word, 128, || false).unwrap(),
                Some(EpsNumber::Integer(expected))
            );
        }
        for &(word, expected) in &[
            (b"-.002".as_slice(), -0.002f32),
            (b"34.5", 34.5),
            (b"123.6e10", 123.6e10),
            (b"1.0E-5", 1e-5),
            (b"1E6", 1e6),
            (b"-1.", -1.),
            (b"2147483648", 2147483648.),
            (b"-2147483649", -2147483649.),
        ] {
            assert_eq!(
                parse_eps_number(word, 128, || false).unwrap(),
                Some(EpsNumber::Real(expected))
            );
        }
        let Some(EpsNumber::Real(zero)) = parse_eps_number(b"-0.0", 128, || false).unwrap() else {
            panic!()
        };
        assert_eq!(zero.to_bits(), (-0.0f32).to_bits());
    }
    #[test]
    fn independent_radix_reference_covers_all_bases_and_signed_bit_boundaries() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/radix-python-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 280);
        for case in cases {
            let word = case["word"].as_str().unwrap();
            let signed = i32::try_from(case["signed"].as_i64().unwrap()).unwrap();
            assert_eq!(
                parse_eps_number(word.as_bytes(), 128, || false).unwrap(),
                Some(EpsNumber::Integer(signed)),
                "{word}"
            );
            assert_eq!(
                parse_eps_number(word.to_ascii_uppercase().as_bytes(), 128, || false).unwrap(),
                Some(EpsNumber::Integer(signed)),
                "{word}"
            );
        }
    }
    #[test]
    fn full_name_grammar_ranges_limits_and_transient_cancellation() {
        let names: &[&[u8]] = &[
            b"",
            b"+",
            b".",
            b"-.",
            b"1e",
            b"1e+",
            b"1.2x",
            b"1 2",
            b"NaN",
            b"inf",
            b"0x10",
            b"1#0",
            b"37#0",
            b"16#",
            b"#F",
            b"16#FG",
            b"+16#FF",
            b"-16#FF",
            b"16#-1",
            b"16##1",
            b"16#100000000x",
            b"9\xff",
        ];
        for &word in names {
            assert_eq!(parse_eps_number(word, 128, || false).unwrap(), None, "{word:?}");
        }
        for word in [
            b"16#100000000".as_slice(),
            b"1e100",
            b"999999999999999999999999999999999999999999999999999999999999",
        ] {
            assert_eq!(parse_eps_number(word, 128, || false), Err(EpsNumberError::Range));
        }
        assert_eq!(parse_eps_number(b"123", 2, || false), Err(EpsNumberError::Limit));
        assert_eq!(
            parse_eps_number(b"123", 3, || true),
            Err(EpsNumberError::Cancelled)
        );
        let mut polls = 0;
        assert_eq!(
            parse_eps_number(b"12345678901234567890", 20, || {
                polls += 1;
                polls == 8
            }),
            Err(EpsNumberError::Cancelled)
        );
        assert_eq!(polls, 8);
    }
}
