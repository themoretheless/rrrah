#!/usr/bin/env python3
"""Frozen CPU LoFTR HDR/alpha complete three-file collection controls."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
OUT=D/'dedup-projected-collection-native-20261011.json'
MANIFEST=D/'dedup-projected-collection-snapshot-20261011.json'
FIXTURES=D/'dedup-projected-loftr-fixtures-20261011.json'
LEGACY=D/'dedup-projected-tiles-native-20261011.json'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE=BASE/'projected-collection-qualified-20261011'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
    m=json.loads(MANIFEST.read_text());f=json.loads(FIXTURES.read_text());work=Path(f['work'])
    old={r['case']:r['evidence'] for r in json.loads(LEGACY.read_text())['results']}
    def verify_source():assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
    verify_source()
    source=ROOT/'target/debug/projected_collection_files';binary=ARCHIVE/source.name;digest=sha(source)
    shutil.copy2(source,binary);assert sha(source)==sha(binary)==digest
    assert m['snapshot'].encode() in binary.read_bytes()
    pins={str(p):sha(p) for p in [MANIFEST,FIXTURES,LEGACY,Path(__file__),binary,WEIGHTS,
        D/'dedup-projected-collection-native-build-20261011.log',D/'dedup-loftr-native-20261010-runtime-pins.json']}
    pins.update(f['fixture_pins']);pins.update(f['source_pins'])
    runtime=json.loads((D/'dedup-loftr-native-20261010-runtime-pins.json').read_text())
    pins.update({p:h for p,h in runtime['pins'].items() if str(TORCH) in p})
    assert sum(str(TORCH) in p for p in pins)==7
    def verify_pins():assert all(sha(p)==h for p,h in pins.items())
    report={'status':'running','producer_pid':os.getpid(),'pins':pins,'source_manifest':str(MANIFEST),'results':[]}
    def save():
        t=OUT.with_suffix('.json.tmp');t.write_text(json.dumps(report,indent=2)+'\n');t.replace(OUT)
    save()
    try:
        for case,paths in [('hdr',['hdr.pfm','hdr-copy.pfm','hdr-collision.pfm']),
                           ('alpha',['alpha.qoi','alpha-copy.qoi','alpha-collision.qoi'])]:
            verify_pins()
            command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),
                *[str(work/p) for p in paths],str(WEIGHTS),'0.05']
            row={'case':case,'status':'running','command':command};report['results'].append(row);save()
            start=time.monotonic();p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            row['process_pid']=p.pid;save();stdout,stderr=p.communicate()
            row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
            assert p.returncode==0,(case,stderr)
            e=json.loads(stdout);row['evidence']=e
            assert e['file_ids']==[0,1,2] and e['managed_used']==0
            pairs=e['pairs'];assert [(x['left'],x['right']) for x in pairs]==[(0,1),(0,2),(1,2)]
            assert [x['supported'] for x in pairs]==[True,False,False]
            assert [len(x['attempts']) for x in pairs]==[1,25,25]
            for x in pairs:
                assert x['pixel_work']==sum(a['pixel_work'] for a in x['attempts'])
                assert all(1<=n<=5 for n in x['prepared_inputs'])
            row['exact_independent_pair_parity']=[
                pairs[0]['attempts']==old[case+'_copy']['attempts'],
                pairs[1]['attempts']==old[case+'_collision']['attempts'],
                pairs[2]['attempts']==old[case+'_collision']['attempts']]
            row['status']='passed';verify_pins();save();print(case,'3 complete pairs, expected support and exhaustion',flush=True)
        verify_source();verify_pins()
        report.update(status='complete_bounded_two_native_projected_collections',collections=2,pairs=6,
            all_expected_outcomes_match=True,
            all_independent_pair_evidence_exact=all(all(r['exact_independent_pair_parity']) for r in report['results']),
            limits=['Six synthetic collection pairs do not qualify broad negative/burst precision or recall',
                    'Native+projected cross-stage portfolio and indexed collection scalability remain open',
                    'No hard Torch tensor/workspace/RSS bound or other-platform Torch qualification'])
    except Exception as e:report.update(status='failed_projected_collection_native_controls',error=str(e));raise
    finally:save()
if __name__=='__main__':main()
