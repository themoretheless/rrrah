#!/usr/bin/env python3
"""Frozen native CPU LoFTR automatic HDR/alpha region-search controls."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
MANIFEST=D/'dedup-projected-tiles-snapshot-20261011.json'
FIXTURES=D/'dedup-projected-loftr-fixtures-20261011.json'
LEGACY=D/'dedup-projected-loftr-identity-native-20261011.json'
OUT=D/'dedup-projected-tiles-native-20261011.json'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE=BASE/'projected-tiles-qualified-20261011'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
    manifest=json.loads(MANIFEST.read_text());fixtures=json.loads(FIXTURES.read_text());work=Path(fixtures['work'])
    legacy=json.loads(LEGACY.read_text());old={r['case']:r for r in legacy['results']}
    def verify_snapshot():
        assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    verify_snapshot()
    source=ROOT/'target/debug/projected_tiles_files';binary=ARCHIVE/'projected_tiles_files';digest=sha(source)
    shutil.copy2(source,binary);assert sha(source)==sha(binary)==digest
    pins={str(p):sha(p) for p in [MANIFEST,FIXTURES,LEGACY,Path(__file__),binary,WEIGHTS,
          D/'dedup-projected-tiles-native-build-20261011.log',D/'dedup-loftr-native-20261010-runtime-pins.json']}
    pins.update(fixtures['fixture_pins']);pins.update(fixtures['source_pins'])
    runtime=json.loads((D/'dedup-loftr-native-20261010-runtime-pins.json').read_text())
    pins.update({p:h for p,h in runtime['pins'].items() if str(TORCH) in p});assert sum(str(TORCH) in p for p in pins)==7
    def verify_pins():assert all(sha(p)==h for p,h in pins.items())
    report={'status':'running','producer_pid':os.getpid(),'pins':pins,'snapshot':str(MANIFEST),
            'results':[],'policy':legacy['policy'],'scope':__doc__}
    def save():
        temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
    save()
    try:
        for case in legacy['results']:
            verify_pins();name=case['case']
            command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),
                     str(work/case['source']),str(work/case['target']),str(WEIGHTS),'0.05']
            row={'case':name,'status':'running','command':command,'expected_supported':case['expected_supported']}
            report['results'].append(row);save();start=time.monotonic()
            process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            row['process_pid']=process.pid;save();stdout,stderr=process.communicate()
            row.update(returncode=process.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
            rss=re.search(r'(\d+)\s+maximum resident set size',stderr)
            if rss:row['maximum_rss_bytes_macos']=int(rss.group(1))
            if process.returncode==0:
                e=json.loads(stdout);row['evidence']=e
                row['expected_outcome_matches']=e['supported']==case['expected_supported']
                assert e['managed_used']==0
                assert 1<=len(e['attempts'])<=25 and all(1<=n<=5 for n in e['prepared_inputs'])
                assert e['pixel_work']==sum(a['pixel_work'] for a in e['attempts'])
                first=e['attempts'][0];prior=old[name]['evidence']
                fields=['supported','original_dimensions','model_digest','raw_proposals','selected_points',
                        'geometry_inliers','pixel_attempts','pixel_work','transform','pixels']
                row['first_attempt_exact_legacy_parity']=all(first[k]==prior[k] for k in fields)
                if not e['supported']:assert len(e['attempts'])==25
                else:assert e['attempts'][-1]['supported']
                row['status']='executed'
            else:row['status']='execution_failed'
            verify_pins();save();print(name,row.get('expected_outcome_matches'),len(row.get('evidence',{}).get('attempts',[])),flush=True)
        verify_snapshot();verify_pins()
        report.update(status='complete_bounded_five_native_projected_tile_controls_execution',
                      all_expected_outcomes_match=all(r.get('expected_outcome_matches',False) for r in report['results']),
                      all_first_attempts_exact_legacy_parity=all(r.get('first_attempt_exact_legacy_parity',False) for r in report['results']),
                      limits=['Synthetic textured fixtures, not arbitrary HDR/alpha or negative/burst corpus qualification',
                              'Generic region-only recovery controls remain separate from native inference results',
                              'No projected portfolio collection or hard tensor/workspace/RSS admission',
                              'No new Linux/Windows Torch execution; shared-host timings are not performance benchmarks'])
    except Exception as error:
        report.update(status='failed_projected_tiles_native_controls',error=str(error));raise
    finally:save()
if __name__=='__main__':main()
