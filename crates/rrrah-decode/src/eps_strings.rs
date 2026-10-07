//! Original PostScript string decoding with exact output admission.
use crate::EpsToken;
use rrrah_core::{BufferError, MemoryBudget, SharedBuffer};

#[derive(Debug, thiserror::Error)]
pub enum EpsStringError {
    #[error("token is not a PostScript string")]
    NotString,
    #[error("invalid PostScript string at payload byte {0}")]
    Syntax(usize),
    #[error("PostScript string output or nesting limit exceeded")]
    Limit,
    #[error("PostScript string decoding cancelled")]
    Cancelled,
    #[error(transparent)]
    Memory(#[from] BufferError),
}
fn space(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}
fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
fn walk<F: FnMut() -> bool>(
    token: EpsToken<'_>,
    limit: usize,
    cancelled: &mut F,
    mut emit: impl FnMut(u8),
) -> Result<usize, EpsStringError> {
    use EpsStringError as E;
    if cancelled() {
        return Err(E::Cancelled);
    }
    let mut count = 0usize;
    let mut put = |b| -> Result<(), E> {
        if count == limit {
            return Err(E::Limit);
        }
        count += 1;
        emit(b);
        Ok(())
    };
    match token {
        EpsToken::String(src) => {
            let mut i = 0;
            let mut depth = 0usize;
            while i < src.len() {
                if cancelled() {
                    return Err(E::Cancelled);
                }
                let at = i;
                let mut b = src[i];
                i += 1;
                if b == b'\\' {
                    b = *src.get(i).ok_or(E::Syntax(at))?;
                    i += 1;
                    b = match b {
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'b' => 8,
                        b'f' => 12,
                        b'\r' | b'\n' => {
                            if b == 13 && src.get(i) == Some(&10) {
                                i += 1;
                            }
                            continue;
                        }
                        b'0'..=b'7' => {
                            let mut value = u16::from(b - b'0');
                            for _ in 0..2 {
                                if let Some(&next) = src.get(i).filter(|b| (b'0'..=b'7').contains(b)) {
                                    value = value * 8 + u16::from(next - b'0');
                                    i += 1;
                                } else {
                                    break;
                                }
                            }
                            (value & 255) as u8
                        }
                        other => other,
                    };
                } else if b == 13 {
                    if src.get(i) == Some(&10) {
                        i += 1;
                    }
                    b = 10;
                } else if b == b'(' {
                    depth += 1;
                    if depth >= 128 {
                        return Err(E::Limit);
                    }
                } else if b == b')' {
                    depth = depth.checked_sub(1).ok_or(E::Syntax(at))?;
                }
                put(b)?;
            }
            if depth != 0 {
                return Err(E::Syntax(src.len()));
            }
        }
        EpsToken::HexString(src) => {
            let mut high = None;
            for (i, &b) in src.iter().enumerate() {
                if cancelled() {
                    return Err(E::Cancelled);
                }
                if space(b) {
                    continue;
                }
                let n = hex(b).ok_or(E::Syntax(i))?;
                if let Some(h) = high.take() {
                    put(h * 16 + n)?;
                } else {
                    high = Some(n);
                }
            }
            if let Some(h) = high {
                put(h * 16)?;
            }
        }
        EpsToken::Ascii85String(src) => {
            let mut value = 0u64;
            let mut digits = 0usize;
            for (i, &b) in src.iter().enumerate() {
                if cancelled() {
                    return Err(E::Cancelled);
                }
                if space(b) {
                    continue;
                }
                if b == b'z' {
                    if digits != 0 {
                        return Err(E::Syntax(i));
                    }
                    for _ in 0..4 {
                        put(0)?;
                    }
                    continue;
                }
                if !(b'!'..=b'u').contains(&b) {
                    return Err(E::Syntax(i));
                }
                value = value * 85 + u64::from(b - b'!');
                digits += 1;
                if digits == 5 {
                    let word = u32::try_from(value).map_err(|_| E::Syntax(i))?;
                    for byte in word.to_be_bytes() {
                        put(byte)?;
                    }
                    value = 0;
                    digits = 0;
                }
            }
            if digits == 1 {
                return Err(E::Syntax(src.len()));
            }
            if digits > 1 {
                for _ in digits..5 {
                    value = value * 85 + 84;
                }
                let word = u32::try_from(value).map_err(|_| E::Syntax(src.len()))?;
                for byte in word.to_be_bytes().iter().take(digits - 1) {
                    put(*byte)?;
                }
            }
        }
        _ => return Err(E::NotString),
    }
    if cancelled() {
        return Err(E::Cancelled);
    }
    Ok(count)
}
pub(crate) fn validate_eps_string<F: FnMut() -> bool>(
    token: EpsToken<'_>,
    max_output_bytes: usize,
    mut cancelled: F,
) -> Result<usize, EpsStringError> {
    walk(token, max_output_bytes, &mut cancelled, |_| {})
}
/// Decode a lexical string payload into shared, budget-owned bytes. A validating
/// first pass computes the exact length without allocation; admission precedes
/// allocation. Cancellation/failure releases the output. Source ownership and its
/// byte accounting remain with the caller. Strings are bytes, not UTF-8 text.
pub fn decode_eps_string<F: FnMut() -> bool>(
    token: EpsToken<'_>,
    max_output_bytes: usize,
    budget: &MemoryBudget,
    mut cancelled: F,
) -> Result<SharedBuffer<u8>, EpsStringError> {
    let length = walk(token, max_output_bytes, &mut cancelled, |_| {})?;
    let mut output = budget.try_buffer(length, 0u8)?;
    let mut index = 0;
    walk(token, length, &mut cancelled, |byte| {
        output[index] = byte;
        index += 1;
    })?;
    Ok(output.freeze())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strings_preserve_binary_values_and_postscript_escape_rules() {
        let cases: &[(EpsToken<'_>, &[u8])] = &[
            (
                EpsToken::String(b"a\r\nb\rc\nd\\\r\nE\\\nF\\n\\r\\t\\b\\f\\(\\)\\\\\\q\\377\\400\\12x\\7"),
                b"a\nb\nc\ndEF\n\r\t\x08\x0c()\\q\xff\0\nx\x07",
            ),
            (EpsToken::String(b"(nested)\0\xff"), b"(nested)\0\xff"),
            (EpsToken::HexString(b"0 a FF 1"), &[10, 255, 16]),
            (
                EpsToken::Ascii85String(b"z !!\0!!! s8W-!"),
                &[0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255],
            ),
            (EpsToken::Ascii85String(b"@ : E ^"), b"abc"),
            (EpsToken::HexString(b""), b""),
        ];
        for &(token, expected) in cases {
            let budget = MemoryBudget::new(expected.len() as u64);
            let decoded = decode_eps_string(token, expected.len(), &budget, || false).unwrap();
            assert_eq!(&*decoded, expected);
            assert_eq!(budget.used(), expected.len() as u64);
            let retained = decoded.clone();
            drop(decoded);
            assert_eq!(budget.used(), expected.len() as u64);
            drop(retained);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn independent_python_ascii85_reference_covers_tail_lengths_and_all_byte_values() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/eps/ascii85-python-reference.json"
        ))
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 40);
        for case in cases {
            let encoded = case["encoded"].as_str().unwrap().as_bytes();
            let expected_hex = case["decoded_hex"].as_str().unwrap().as_bytes();
            let expected: Vec<u8> = expected_hex
                .chunks_exact(2)
                .map(|p| hex(p[0]).unwrap() * 16 + hex(p[1]).unwrap())
                .collect();
            let budget = MemoryBudget::new(expected.len() as u64);
            let decoded =
                decode_eps_string(EpsToken::Ascii85String(encoded), expected.len(), &budget, || {
                    false
                })
                .unwrap();
            assert_eq!(&*decoded, &expected, "length {}", expected.len());
            drop(decoded);
            assert_eq!(budget.used(), 0);
        }
    }
    #[test]
    fn source_and_decoded_string_share_root_admission_and_last_owner_lifetime() {
        for root_limit in [6, 7] {
            let root = MemoryBudget::new(root_limit);
            let mut source = root.try_buffer(4, 0u8).unwrap();
            source.copy_from_slice(b"@:E^");
            let output_budget = root.child(3);
            let result = decode_eps_string(EpsToken::Ascii85String(&source), 3, &output_budget, || false);
            if root_limit == 6 {
                assert!(matches!(result, Err(EpsStringError::Memory(_))));
                assert_eq!(root.used(), 4);
                assert_eq!(output_budget.used(), 0);
            } else {
                let decoded = result.unwrap();
                assert_eq!(&*decoded, b"abc");
                assert_eq!(root.used(), 7);
                assert_eq!(root.peak(), 7);
                let retained = decoded.clone();
                drop(decoded);
                drop(source);
                assert_eq!(root.used(), 3);
                drop(retained);
                assert_eq!(root.used(), 0);
                continue;
            }
            drop(source);
            assert_eq!(root.used(), 0);
        }
    }
    #[test]
    fn malformed_limits_and_transient_cancellation_do_not_leak_admission() {
        let invalid = [
            EpsToken::String(b"\\"),
            EpsToken::String(b"("),
            EpsToken::String(b")"),
            EpsToken::HexString(b"G"),
            EpsToken::Ascii85String(b"!z"),
            EpsToken::Ascii85String(b"!"),
            EpsToken::Ascii85String(b"uuuuu"),
            EpsToken::Ascii85String(b"uuuu"),
            EpsToken::Ascii85String(b"v"),
        ];
        let budget = MemoryBudget::new(64);
        for token in invalid {
            assert!(matches!(
                decode_eps_string(token, 64, &budget, || false),
                Err(EpsStringError::Syntax(_))
            ));
            assert_eq!(budget.used(), 0);
        }
        assert!(matches!(
            decode_eps_string(EpsToken::Word(b"x"), 64, &budget, || false),
            Err(EpsStringError::NotString)
        ));
        assert!(matches!(
            decode_eps_string(EpsToken::Ascii85String(b"z"), 3, &budget, || false),
            Err(EpsStringError::Limit)
        ));
        assert!(matches!(
            decode_eps_string(EpsToken::String(b"123"), 3, &MemoryBudget::new(2), || false),
            Err(EpsStringError::Memory(_))
        ));
        // First pass has initial + 3 input + final polls; cancel during second pass.
        let mut polls = 0;
        assert!(matches!(
            decode_eps_string(EpsToken::String(b"abc"), 3, &budget, || {
                polls += 1;
                polls == 7
            }),
            Err(EpsStringError::Cancelled)
        ));
        assert_eq!(polls, 7);
        assert_eq!(budget.used(), 0);
    }
}
