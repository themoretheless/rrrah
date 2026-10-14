#!/usr/bin/env python3
"""Compare the exact source Duration arithmetic to Python's arbitrary precision."""
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import random
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'crates/rrrah-dedup/src/sequence.rs'
OUT = ROOT / 'docs/research/dedup-duration-fraction-oracle-20261014'
OUT.mkdir(exist_ok=False)
text = SOURCE.read_text()
definition = text[text.index('/// Exact rational duration'):text.index('#[cfg(all(test, feature = "decode"))]')]
driver = r'''
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        let values: Vec<u64> = line.unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
        let a = Duration::new(values[0], values[1]).unwrap();
        let b = Duration::new(values[2], values[3]).unwrap();
        for result in [a.checked_add(b), a.subtract(b)] {
            match result {
                Ok(value) => print!("{} {} ", value.numerator, value.denominator),
                Err(_) => print!("E E "),
            }
        }
        println!("{}", a.less_or_equal(b));
    }
}
'''
rust = OUT / 'source-arithmetic.rs'
rust.write_text('#![allow(dead_code)]\n#[derive(Debug)] pub enum SequenceError { Timing }\n' + definition + driver)
binary = Path('/private/tmp/rrrah-duration-fraction-oracle-20261014')
assert not binary.exists()
build = subprocess.run(['rustc', '--edition', '2024', '--cfg', 'feature="decode"', '-O', str(rust), '-o', str(binary)], capture_output=True, text=True)
(OUT / 'build.log').write_text(build.stdout + build.stderr)
assert build.returncode == 0
maximum = 2**64 - 1
rng = random.Random(173)
cases = [(maximum - 2, maximum - 1, maximum - 2, maximum - 1)]
edges = [0, 1, 2, 3, 2**32 - 1, 2**32, 2**63 - 1, 2**63, maximum - 2, maximum - 1, maximum]
for a in edges:
    for b in edges[1:]:
        for c in edges:
            for d in edges[1:]:
                cases.append((a, b, c, d))
for _ in range(20000):
    cases.append((rng.getrandbits(64), rng.randrange(1, maximum + 1), rng.getrandbits(64), rng.randrange(1, maximum + 1)))
for _ in range(10000):
    denominator = rng.randrange(1, maximum + 1)
    cases.append((rng.getrandbits(64), denominator, rng.getrandbits(64), denominator))
input_data = ''.join(' '.join(map(str, case)) + '\n' for case in cases)
(OUT / 'cases.txt').write_text(input_data)
run = subprocess.run([str(binary)], input=input_data, capture_output=True, text=True)
assert run.returncode == 0, run.stderr
(OUT / 'results.txt').write_text(run.stdout)
rows = run.stdout.splitlines()
assert len(rows) == len(cases)
valid = dict(add=0, subtract=0)
refused = dict(add=0, subtract=0)
for index, ((a, b, c, d), row) in enumerate(zip(cases, rows)):
    left, right = Fraction(a, b), Fraction(c, d)
    fields = row.split()
    assert len(fields) == 5
    for operation, value, offset in [('add', left + right, 0), ('subtract', left - right, 2)]:
        fits = 0 <= value.numerator <= maximum and value.denominator <= maximum
        expected = [str(value.numerator), str(value.denominator)] if fits else ['E', 'E']
        assert fields[offset:offset + 2] == expected, (index, operation, (a, b, c, d), expected, fields)
        (valid if fits else refused)[operation] += 1
    assert fields[4] == str(left <= right).lower(), index
assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == hashlib.sha256(text.encode()).hexdigest()
report = dict(status='passed', cases=len(cases), seed=173, valid=valid, refused=refused, source_sha256=hashlib.sha256(SOURCE.read_bytes()).hexdigest(), oracle='Python fractions.Fraction with arbitrary-precision integers; exact representability in nonnegative u64 rational duration', scope='Original source Duration implementation compiled with minimal Timing error type. Addition, subtraction and ordering only; no decoder, whole crate or timeline integration claim.', pins={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.iterdir() if p.is_file()})
(OUT / 'audit.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({k: report[k] for k in ['status', 'cases', 'valid', 'refused']}))
