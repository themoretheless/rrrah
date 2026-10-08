"""Adversarial tests for the audit parser, not a native collection qualification."""
import copy, hashlib, json, subprocess, sys, tempfile
from pathlib import Path

baseline = Path('docs/research/dedup-original-managed-region-200201.json')
base = json.loads(baseline.read_text())
base['input_hashes'][str(baseline)] = hashlib.sha256(baseline.read_bytes()).hexdigest()
verifier = Path('scripts/verify-dedup-managed-real-parity.py')
out = Path('docs/research/dedup-managed-real-verifier-adversaries.json')
assert not out.exists()


def invalid_index(r):
    r['evidence']['inliers'][0] = len(r['evidence']['correspondences'])


def invalid_pixel_counts(r):
    r['evidence']['regions'][0]['pixels']['counts'][0][0] = 999999999


def acceptance_flip(r):
    r['evidence']['regions'][0]['accepted'] = True


def model_outlier(r):
    r['evidence']['matrix'][0][2] += 10000


cases = [
    ('invalid_inlier', invalid_index),
    ('impossible_pixel_counts', invalid_pixel_counts),
    ('unsupported_acceptance', acceptance_flip),
    ('model_outlier', model_outlier),
    ('boolean_managed_used', lambda r: r['evidence'].update(managed_used=False)),
    ('unreleased_payload', lambda r: r['evidence'].update(managed_used=1)),
    ('over_budget_peak', lambda r: r['evidence'].update(managed_peak=512*1024*1024+1)),
    ('whole_identity_claim', lambda r: r['evidence'].update(whole_candidate=True)),
    ('false_source_pin', lambda r: r['input_hashes'].update({str(baseline): '0'*64})),
]
results = []
with tempfile.TemporaryDirectory(prefix='dedup-managed-audit-') as tmp:
    tmp = Path(tmp)
    def run(r, name):
        source = tmp / (name + '.json')
        target = tmp / (name + '-audit.json')
        source.write_text(json.dumps(r, allow_nan=False))
        result = subprocess.run([sys.executable, str(verifier), str(source), str(target)], capture_output=True, text=True)
        return result.returncode, target.exists()
    code, saved = run(base, 'parser_control_from_file_evidence')
    assert code == 0 and saved
    for name, mutate in cases:
        r = copy.deepcopy(base)
        mutate(r)
        r['stdout'] = json.dumps(r['evidence'], allow_nan=False)
        code, saved = run(r, name)
        assert code != 0 and not saved, name
        results.append({'case': name, 'rejected': True})
    r = copy.deepcopy(base)
    r['evidence']['region_support_count'] = 2
    code, saved = run(r, 'raw_output_disagreement')
    assert code != 0 and not saved
    results.append({'case': 'raw_output_disagreement', 'rejected': True})
out.write_text(json.dumps({
    'status': 'verified_parser_adversaries', 'rejected_cases': results,
    'input_hashes': {str(f): hashlib.sha256(f.read_bytes()).hexdigest() for f in [baseline, verifier, Path(__file__)]},
    'scope': 'Audit parser control derives from existing native file evidence, not a collection run. Ten corrupted records rejected without publishing audit output.'
}, indent=2) + '\n')
print('parser control passed; 10 corrupted records rejected')
