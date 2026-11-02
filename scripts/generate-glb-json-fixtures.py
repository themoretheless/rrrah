#!/usr/bin/env python3
"""CC0 authored JSON controls, classified by Python stdlib independently of Rust."""
import decimal
import json
from pathlib import Path


def check_strings(value):
    if isinstance(value, str):
        value.encode('utf-8')  # native policy: Unicode scalar strings, no lone surrogates
    elif isinstance(value, list):
        for item in value:
            check_strings(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            check_strings(key)
            check_strings(item)


def tokens(value):
    if isinstance(value, dict):
        return 1 + sum(1 + tokens(item) for item in value.values())
    if isinstance(value, list):
        return 1 + sum(tokens(item) for item in value)
    return 1


def reject_constant(value):
    raise ValueError(value)


def classify(data):
    try:
        value = json.loads(data, parse_float=decimal.Decimal, parse_constant=reject_constant)
        check_strings(value)
        # Duplicate keys are legal controls: Python collapses earlier values,
        # while native tape preserves all source tokens, so omit token census.
        return 1, tokens(value)
    except (ValueError, UnicodeError):
        return 0, 0


base = b'{"asset":{"version":"2.0"},"values":[1,true,null,"cube"]}'
cases = [base[:end] for end in range(len(base)+1)]
cases += [b'null', b'true', b'false', b'0', b'-0', b'0.1', b'1e2', b'1E-2',
          b'18446744073709551615', b'9007199254740993.0', b'1e999999',
          b'"\\uD83D\\uDE00"', '"куб/立方體/😀"'.encode(),
          b'"\\b\\f\\n\\r\\t\\\\\\/\\\""', b'\xef\xbb\xbf{}',
          b'[]', b'{}', b'[{},[],[1,2],{"a":[3]}]',
          b'01', b'1.', b'+1', b'-.1', b'1e', b'1e+', b'NaN', b'Infinity',
          b'[1,]', b'{"x":1,}', b'{1:2}', b'{}{}', b'"\\q"', b'"\\uXXXX"',
          b'"\\uD800"', b'"\\uDC00"', b'"\\uD800\\u0000"', b'"\xff"',
          b'"\x01"', b'\xef\xbb\xbf\xef\xbb\xbf{}', b'/*x*/{}', b'null\0']
# A bounded deterministic mutation matrix, independent oracle decides outcomes.
for at in range(len(base)):
    for replacement in (0, 34, 92, 255):
        cases.append(base[:at]+bytes([replacement])+base[at+1:])
rows = []
for data in dict.fromkeys(cases):
    valid, count = classify(data)
    rows.append(f'{valid}\t{count}\t{data.hex()}\n')
path = Path(__file__).resolve().parents[1]/'tests/fixtures/glb/json-corpus.tsv'
path.write_text(''.join(rows))
print(f'{len(rows)} independent JSON controls written')

# Exact decimal arithmetic oracle for schema integer fields, including values
# above the binary64 consecutive-integer range and unsigned 64-bit boundaries.
values = ['-0', '-0.0e999999999', '0e-999999999', '1e999999999',
          '9007199254740993.0', '18446744073709551615.000',
          '184467440737095516150e-1', '0.00100e3']
for value in [0, 1, 99, 2**53-1, 2**53, 2**53+1, 2**64-1, 2**64, 2**64+1]:
    values += [str(value), f'{value}.0', (f'{value}00e-2' if value else '0e-2'), f'{value}e+0', f'-{value}', f'{value}.5']
rows = []
for text in dict.fromkeys(values):
    # Decimal accepts leading-zero forms that JSON forbids; validate both.
    json.loads(text, parse_float=decimal.Decimal, parse_constant=reject_constant)
    number = decimal.Decimal(text)
    integer = number.to_integral_value(rounding=decimal.ROUND_DOWN)
    expected = str(int(number)) if 0 <= number <= 2**64-1 and number == integer else '-'
    rows.append(f'{text}\t{expected}\n')
path.with_name('json-integers.tsv').write_text(''.join(rows))
print(f'{len(rows)} independent exact integer controls written')
