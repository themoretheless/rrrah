#!/usr/bin/env python3
"""Frozen local CPU LoFTR complete collections with distinct exact/visual/unknown pairs."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-dependency-observation-snapshot-20261011.json'
FIXTURES=D/'dedup-projected-loftr-fixtures-20261011.json'
PRIOR=D/'dedup-exact-loftr-native-20261011.json'
OUT=D/'dedup-dependency-observation-native-20261011.json'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE=BASE/'dependency-observation-collection-qualified-20261011'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot'])
 def verify_source():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify_source();f=json.loads(FIXTURES.read_text());work=Path(f['work'])
 prior={r['case']:r['evidence']['result'] for r in json.loads(PRIOR.read_text())['results'] if 'evidence' in r}
 source=ROOT/'target/debug/exact_collection_files';binary=ARCHIVE/source.name;digest=sha(source)
 shutil.copy2(source,binary);assert sha(source)==sha(binary)==digest
 assert m['snapshot'].encode() in binary.read_bytes()
 data=(work/'alpha.qoi').read_bytes();assert data[14]==255 and data[18]==255
 alt=ARCHIVE/'alpha-reencoded.qoi';alt.write_bytes(data[:14]+bytes([254])+data[15:18]+data[19:])
 unknown=[ARCHIVE/('unknown-'+str(i)) for i in range(3)]
 for p in unknown:p.write_bytes(b'undecodable exact collection copies\x00\xff')
 missing=ARCHIVE/'absent-weights';assert not missing.exists()
 pins={str(p):sha(p) for p in [MANIFEST,FIXTURES,PRIOR,Path(__file__),binary,WEIGHTS,alt,*unknown,D/'dedup-dependency-observation-native-build-20261011.log']}
 pins.update(f['fixture_pins']);pins.update(f['source_pins'])
 runtime_path=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(runtime_path.read_text());pins[str(runtime_path)]=sha(runtime_path)
 pins.update({p:h for p,h in runtime['pins'].items() if str(TORCH) in p});assert sum(str(TORCH) in p for p in pins)==7
 def verify_pins():assert all(sha(p)==h for p,h in pins.items())
 cases=[('empty','native-first',missing,[],'exact'),('singleton','model-first',missing,unknown[:1],'exact'),
        ('unknown_copies','native-first',missing,unknown,'exact'),
        ('alpha_copies','model-first',missing,[work/'alpha.qoi',work/'alpha-copy.qoi',work/'alpha.qoi'],'exact')]
 for order in ['native-first','model-first']:
  cases.append(('mixed_visual_'+order,order,WEIGHTS,[work/'alpha.qoi',work/'alpha-copy.qoi',alt],'visual'))
 cases += [('mixed_unknown','native-first',WEIGHTS,[work/'alpha.qoi',work/'alpha-copy.qoi',work/'alpha-collision.qoi'],'unknown'),
           ('missing_weights_after_exact','model-first',missing,[work/'alpha.qoi',work/'alpha-copy.qoi',alt],'missing'),
           ('file_limit_before_weights','native-first',missing,[*unknown,unknown[0]],'files')]
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'source_manifest':str(MANIFEST),'results':[],'expected_invocations':len(cases),'scope':__doc__}
 def save():
  t=OUT.with_suffix('.json.tmp');t.write_text(json.dumps(report,indent=2)+'\n');t.replace(OUT)
 save()
 try:
  for name,order,weights,paths,expected in cases:
   verify_pins();assert not missing.exists()
   command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),str(weights),order,*map(str,paths)]
   row={'case':name,'status':'running','command':command,'expected':expected};report['results'].append(row);save();start=time.monotonic()
   p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
   stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
   log=ARCHIVE/(name+'.log');log.write_text(stdout+stderr);row.update(log=str(log),log_sha256=sha(log))
   if expected in ['missing','files']:
    assert p.returncode!=0 and not stdout.strip(),(name,stdout,stderr)
    assert ('No such file' in stderr if expected=='missing' else ('Files' in stderr or 'file limit' in stderr)),stderr
   else:
    assert p.returncode==0,(name,stdout,stderr)
    output=json.loads(stdout);assert output['managed_used']==0;e=output['result'];row['evidence']=output
    assert e['file_ids']==list(range(len(paths)))
    pairs=e['pairs'];assert [(r['left'],r['right']) for r in pairs]==[(i,j) for i in range(len(paths)) for j in range(i+1,len(paths))]
    if expected=='exact':assert all(r['evidence']=={'outcome':'exact_bytes'} for r in pairs)
    else:
     assert pairs[0]['evidence']=={'outcome':'exact_bytes'}
     control=prior['visual_alpha_'+order] if expected=='visual' else prior['collision_alpha_native-first']
     assert all(r['evidence']==control for r in pairs[1:]),name
     row['independent_pair_evidence_exact']=True
   row['status']='passed';save();print(name,'passed',flush=True)
  verify_source();verify_pins()
  report.update(status='complete_bounded_frozen_dependency_observation_native_controls',invocations=len(cases),all_expected_outcomes_match=True)
 except Exception as e:report.update(status='failed_exact_first_loftr_collection_controls',error=str(e));raise
 finally:save()
if __name__=='__main__':main()
