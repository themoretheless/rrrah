#!/usr/bin/env python3
"""Independently freeze and audit a completed current458 prefix, including unknowns."""
import hashlib
import json
import re
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    report = D/'dedup-current-native-bank-all-positives-20261012.json'
    raw = report.read_bytes(); r = json.loads(raw)
    n = r['completed']
    assert 0 < n <= 458 and len(r['results'])==n and len(r['plan'])==r['required']==458
    assert r['build_returncode']==0
    assert all(sha(p)==h for p,h in r['pins'].items())
    m = json.loads(Path(r['source_manifest']).read_text())
    assert all(sha(Path(r['source'])/p)==h for p,h in m['source_hashes'].items())
    ordering = T/'rrrah-loftr-publisher-positives-qualified-20261011/audits/ordering-prefix-0458.json'
    b = json.loads(ordering.read_text())
    priority = [3,7,27,57,67,153,255]
    indices = priority+[i for i in range(458) if i not in priority]
    assert r['plan']==[dict(b['plan'][i],baseline_index=i) for i in indices]
    old = T/'rrrah-native-bank-outcome-native-harness-20261011'
    h = Path(r['harness'])
    assert (h/'src/bin/native_bank_current_all_positives.rs').read_bytes()==(old/'src/bin/native_bank_outcome_files.rs').read_bytes()
    assert (h/'src/native_policy.rs').read_bytes()==(old/'src/native_policy.rs').read_bytes()
    assert (h/'Cargo.lock').read_bytes()==(old/'Cargo.lock').read_bytes()
    assert (h/'Cargo.toml').read_text()==(old/'Cargo.toml').read_text().replace(
        str(T/'rrrah-native-bank-outcome-native-snapshot-20261011'),r['source'])
    counts = dict(supported=0,inconclusive=0,rejected=0,execution_failed=0)
    rows=[]; pins=dict(r['pins'])
    for index,row in enumerate(r['results']):
        path=Path(row['report']); assert sha(path)==row['sha256']
        v=json.loads(path.read_text()); item=r['plan'][index]
        assert v['index']==row['index']==index and v['baseline_index']==row['baseline_index']==item['baseline_index']
        expected=['/usr/bin/time','-l','/usr/bin/env',
            'DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),
            r['executable'],item['left'],item['right'],
            str(T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors'),
            '640',str(item['source_tolerance']),'1000000']
        assert v['command']==expected
        kind='execution_failed'
        if v['returncode']==0:
            value=json.loads(v['stdout'])
            assert value['files']==[2,9] and value['managed_used']==0 and value['opposite_trials']==1000000
            assert len(value['pairs'])==1
            pair=value['pairs'][0]
            assert (pair['left'],pair['right'])==(2,9) and type(pair['model_attempted']) is bool
            support=pair['support']; assert support is None or type(support) is bool
            assert support is not None or (pair['model_attempted'] and pair['detail']['terminal']=='inconclusive')
            kind='supported' if support is True else ('rejected' if support is False else 'inconclusive')
            assert int(re.search(r'(\d+)\s+maximum resident set size',v['stderr'])[1])==v['rss_bytes']
        assert kind==v['status']==row['status']
        counts[kind]+=1
        rows.append(dict(index=index,baseline_index=item['baseline_index'],status=kind))
        pins[str(path)]=row['sha256']
    assert counts==r['counts']
    capture=T/f'rrrah-current-native-bank-prefix-{n:04d}-20261012.json'
    out=D/f'dedup-current-native-bank-prefix-{n:04d}-audit-20261012.json'
    assert not capture.exists() and not out.exists()
    capture.write_bytes(raw)
    pins.update({str(p):sha(p) for p in [capture,Path(__file__)]})
    out.write_text(json.dumps(dict(status='verified_current_native_bank_completed_prefix',
        completed=n,required=458,counts=counts,rows=rows,pins=pins,
        scope='Completed prefix only on qualified frozen current2696-file snapshot; immutable case JSONs and exact original caller/manifest-path-only policy/ordering verified. Unknown/reject/error distinct. Full458 completion requires terminal producer and all458cases; no broad negatives/fullformats/platform/full500/RSS cap claim.'),indent=2)+'\n')
    print(n,counts)

if __name__=='__main__':
    main()
