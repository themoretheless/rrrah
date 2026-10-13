#!/usr/bin/env python3
"""Run unchanged pinned upstream tests, then authored public-API counterexamples."""
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

CONTROLS = r'''
use fast_dhash::Dhash;

#[test]
fn sub_cell_width_discards_all_content() {
    let a: Vec<u8> = (0..64).map(|i| (i % 8) * 30).collect();
    let b: Vec<u8> = a.iter().map(|v| 255 - v).collect();
    assert_ne!(a, b);
    assert_eq!(Dhash::new(&a, 8, 8, 1).hash, 0);
    assert_eq!(Dhash::new(&b, 8, 8, 1).hash, 0);
}

#[test]
fn sub_cell_height_discards_all_content() {
    let a: Vec<u8> = (0..63).map(|i| (i % 9) * 28).collect();
    let b: Vec<u8> = a.iter().map(|v| 255 - v).collect();
    assert_eq!(Dhash::new(&a, 9, 7, 1).hash, 0);
    assert_eq!(Dhash::new(&b, 9, 7, 1).hash, 0);
}

#[test]
fn last_column_and_last_row_remainders_are_not_hashed() {
    for (w, h) in [(10usize, 8usize), (9, 9)] {
        let a = vec![0; w*h];
        let mut b = a.clone();
        for y in 0..h { for x in 0..w {
            if (w == 10 && x == w-1) || (h == 9 && y == h-1) { b[y*w+x] = 255; }
        }}
        assert_ne!(a, b);
        assert_eq!(Dhash::new(&a, w as u32, h as u32, 1).hash,
                   Dhash::new(&b, w as u32, h as u32, 1).hash);
    }
}

#[test]
fn invisible_rgba_rgb_changes_all_hash_bits() {
    let mut a = Vec::new(); let mut b = Vec::new();
    for _ in 0..8 { for x in 0u8..9 {
        a.extend_from_slice(&[x*28, x*28, x*28, 0]);
        let value = 255-x*28;
        b.extend_from_slice(&[value, value, value, 0]);
    }}
    let left = Dhash::new(&a, 9, 8, 4);
    let right = Dhash::new(&b, 9, 8, 4);
    assert_eq!(left.hash, 0);
    assert_eq!(right.hash, u64::MAX);
    assert_eq!(left.hamming_distance(&right), 64);
}

#[test]
fn approximate_partial_eq_is_nontransitive() {
    let a = Dhash { hash: 0 };
    let b = Dhash { hash: (1<<10)-1 };
    let c = Dhash { hash: (1<<20)-1 };
    assert_eq!(a.hamming_distance(&b), 10);
    assert_eq!(b.hamming_distance(&c), 10);
    assert_eq!(a.hamming_distance(&c), 20);
    assert!(a == b && b == c && a != c);
}

#[test]
fn opaque_black_and_white_share_zero_hash() {
    assert_eq!(Dhash::new(&vec![0; 9*8*3], 9, 8, 3).hash, 0);
    assert_eq!(Dhash::new(&vec![255; 9*8*3], 9, 8, 3).hash, 0);
}

#[test]
fn invalid_buffer_length_panics_before_pixel_loops() {
    assert!(std::panic::catch_unwind(|| Dhash::new(&[0; 3], 9, 8, 3)).is_err());
}
'''

def main():
    manifest = D / 'fast-dhash-full-source-20261012.json'
    m = json.loads(manifest.read_text())
    source = Path(m['source'])
    assert all(sha(source / n) == h for n, h in m['files'].items())
    harness = T / 'rrrah-fast-dhash-controls-20261012'
    out = D / 'fast-dhash-runtime-controls-20261012.json'
    assert not harness.exists() and not out.exists()
    shutil.copytree(source, harness)
    control = harness / 'tests/rrrah_controls.rs'
    control.parent.mkdir()
    control.write_text(CONTROLS)
    pins = {str(p):sha(p) for p in [manifest, Path(__file__), control]}
    pins.update({str(source/n):h for n,h in m['files'].items()})
    r = dict(status='running', producer_pid=os.getpid(), source=str(source), harness=str(harness),
             results=[], pins=pins, scope='Original pinned Cargo manifest/lock/source and fixtures unchanged. Three embedded units and one doctest attempted first; seven authored public-API controls second. No zero-channel unsafe case executed, benchmark, root adoption or all500 completion.')
    def save():
        tmp = out.with_suffix('.tmp')
        tmp.write_text(json.dumps(r,indent=2)+'\n')
        tmp.replace(out)
    save()
    for label, cwd, extra in [('upstream', source, []), ('controls', harness, ['--test','rrrah_controls'])]:
        cmd = ['cargo','test','--locked','--jobs','2','--target-dir',str(T/'rrrah-fast-dhash-target-20261012'),*extra]
        log = T/f'rrrah-fast-dhash-{label}-20261012.log'
        assert not log.exists()
        with log.open('w') as output:
            child = subprocess.Popen(cmd,cwd=cwd,stdout=output,stderr=subprocess.STDOUT)
            r.update(active_phase=label,child_pid=child.pid)
            save()
            rc = child.wait()
        r['pins'][str(log)] = sha(log)
        row = dict(phase=label, command=cmd, cwd=str(cwd), returncode=rc, log=str(log))
        r['results'].append(row)
        assert all(sha(source/n)==sha(harness/n)==h for n,h in m['files'].items())
        if rc:
            r['status']='terminal_build_or_test_failure'
            save()
            return
        text = log.read_text()
        summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text)
        expected = [('3','0','0','0','0'),('1','0','0','0','0')] if label=='upstream' else [('7','0','0','0','0')]
        assert summaries==expected
        row['summaries'] = summaries
        for executable in re.findall(r'Running (?:unittests src/lib\.rs|tests/rrrah_controls\.rs) \(([^)]+)\)',text):
            saved = T/f'rrrah-fast-dhash-{label}-executable-20261012'
            assert not saved.exists()
            shutil.copy2(executable,saved)
            row['saved_executable']=str(saved)
            r['pins'][str(saved)]=sha(saved)
        save()
    assert all(sha(p)==h for p,h in r['pins'].items())
    r['status']='complete_original3_doctest1_controls7'
    save()
    print(r['status'])

if __name__=='__main__':
    main()
