#!/usr/bin/env python3
"""Adopt only the exact native source qualified by units and full A/B controls."""
import hashlib
import json
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    paths = [D / name for name in [
        'dedup-loftr-position-floor-candidate-20261012.json',
        'dedup-loftr-position-floor-complete-audit-20261012.json',
        'dedup-loftr-position-positive-complete-audit-20261012.json',
        'dedup-loftr-current-stack-runtime-20261012.json',
    ]]
    units, worst, positives, baseline = [json.loads(p.read_text()) for p in paths]
    assert units['status'] == 'isolated_57_units_passed_not_adopted'
    assert (units['passed'], units['failed'], units['ignored']) == (57, 0, 0)
    assert worst['status'] == 'verified_complete_candidate_not_adopted'
    assert worst['eligible_for_fixed_case_adoption'] and worst['entire_evidence_identical']
    assert positives['status'] == 'verified_two_complete_positive_controls'
    assert positives['all_supported'] and positives['entire_evidence_identical']
    for report in [units, worst, positives, baseline]:
        assert all(sha(p) == value for p, value in report['pins'].items())
    native = R / 'vendor/loftr/src'
    frozen = Path(baseline['source']) / 'vendor/loftr/src'
    qualified = Path(units['source']) / 'src'
    before = {str(p.relative_to(frozen)): sha(p) for p in frozen.rglob('*') if p.is_file()}
    current = {str(p.relative_to(native)): sha(p) for p in native.rglob('*') if p.is_file()}
    tested = {str(p.relative_to(qualified)): sha(p) for p in qualified.rglob('*') if p.is_file()}
    assert before == current, 'Native sources changed since the full baseline snapshot'
    assert before.keys() == tested.keys()
    changed = ['loftr_model.rs', 'position_encoding/tests.rs']
    assert sorted(n for n in before if before[n] != tested[n]) == changed
    out = D / 'dedup-loftr-position-floor-adoption-20261012.json'
    assert not out.exists()
    for name in changed:
        (native / name).write_bytes((qualified / name).read_bytes())
    after = {name: sha(native / name) for name in before}
    assert after == tested
    out.write_text(json.dumps(dict(
        status='adopted_exact_57_unit_qualified_native_source',
        production_delta=['loftr_model.rs'], test_delta=['position_encoding/tests.rs'],
        before=before, after=after, native_units_passed=57,
        worst_pair_rss_bytes=worst['rss_bytes'],
        baseline_rss_bytes=worst['baseline_rss_bytes'],
        rss_delta_bytes=worst['rss_delta_bytes'],
        entire_evidence_identical=True, positive_controls_supported=2,
        pins={str(p): sha(p) for p in [*paths, Path(__file__)]},
        scope='Exact tested native bytes adopted. One heavy negative and two precommitted positives with complete JSON parity. No universal RSS limit, latency guarantee, full458 current-stack or full500 review claim.',
    ), indent=2) + '\n')
    print('Adopted exact qualified position floor source')

if __name__ == '__main__':
    main()
