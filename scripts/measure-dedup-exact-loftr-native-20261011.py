#!/usr/bin/env python3
"""Frozen local CPU LoFTR exact-first controls with independent visual parity."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-exact-loftr-snapshot-20261011.json'
FIXTURES=D/'dedup-projected-loftr-fixtures-20261011.json'
PRIOR=D/'dedup-projected-portfolio-native-20261011.json'
OUT=D/'dedup-exact-loftr-native-20261011.json'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE=BASE/'exact-loftr-qualified-20261011'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot'])
 def verify_source():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify_source();f=json.loads(FIXTURES.read_text());work=Path(f['work'])
 prior={r['case']:r['evidence']['result'] for r in json.loads(PRIOR.read_text())['results']}
 source=ROOT/'target/debug/exact_portfolio_files';binary=ARCHIVE/source.name;digest=sha(source)
 shutil.copy2(source,binary);assert sha(source)==sha(binary)==digest
 assert m['snapshot'].encode() in binary.read_bytes()
 # Re-encode the first opaque QOI RGBA opcode as RGB. QOI starts with alpha=255.
 data=(work/'alpha.qoi').read_bytes();assert data[14]==255 and data[18]==255
 alpha_alt=ARCHIVE/'alpha-reencoded.qoi';alpha_alt.write_bytes(data[:14]+bytes([254])+data[15:18]+data[19:])
 assert sha(alpha_alt)!=sha(work/'alpha.qoi')
 hdr_alt=ARCHIVE/'hdr-reencoded.pfm';data=(work/'hdr.pfm').read_bytes();assert b'-1.0\n' in data[:32]
 hdr_alt.write_bytes(data.replace(b'-1.0\n',b'-1.00\n',1))
 unknown=[ARCHIVE/'undecodable-a',ARCHIVE/'undecodable-b']
 for p in unknown:p.write_bytes(b'exact byte copies without a supported decoder\x00\xff')
 missing=ARCHIVE/'absent-weights';assert not missing.exists()
 pins={str(p):sha(p) for p in [MANIFEST,FIXTURES,PRIOR,Path(__file__),binary,WEIGHTS,alpha_alt,hdr_alt,*unknown,D/'dedup-exact-loftr-native-build-20261011.log']}
 pins.update(f['fixture_pins']);pins.update(f['source_pins'])
 runtime_path=D/'dedup-loftr-native-20261010-runtime-pins.json';runtime=json.loads(runtime_path.read_text());pins[str(runtime_path)]=sha(runtime_path)
 pins.update({p:h for p,h in runtime['pins'].items() if str(TORCH) in p});assert sum(str(TORCH) in p for p in pins)==7
 def verify_pins():assert all(sha(p)==h for p,h in pins.items())
 cases=[]
 for order in ['native-first','model-first']:
  cases += [('exact_alpha_'+order,order,missing,work/'alpha.qoi',work/'alpha-copy.qoi','exact',None),
            ('exact_undecodable_'+order,order,missing,*unknown,'exact',None),
            ('visual_alpha_'+order,order,WEIGHTS,work/'alpha.qoi',alpha_alt,'visual','alpha_copy_'+order),
            ('collision_alpha_'+order,order,WEIGHTS,work/'alpha.qoi',work/'alpha-collision.qoi','unknown','alpha_collision_'+order)]
 cases += [('exact_hdr','native-first',missing,work/'hdr.pfm',work/'hdr-copy.pfm','exact',None),
           ('visual_hdr','native-first',WEIGHTS,work/'hdr.pfm',hdr_alt,'visual','hdr_copy_native-first'),
           ('visual_missing_weights','model-first',missing,work/'alpha.qoi',alpha_alt,'error',None)]
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'source_manifest':str(MANIFEST),'results':[],'expected_invocations':len(cases),'scope':__doc__}
 def save():
  t=OUT.with_suffix('.json.tmp');t.write_text(json.dumps(report,indent=2)+'\n');t.replace(OUT)
 save()
 try:
  for name,order,weights,left,right,expected,control in cases:
   verify_pins();assert not missing.exists()
   command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),str(weights),order,str(left),str(right)]
   row={'case':name,'status':'running','command':command,'expected':expected};report['results'].append(row);save();start=time.monotonic()
   p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
   stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
   log=ARCHIVE/(name+'.log');log.write_text(stdout+stderr);row.update(log=str(log),log_sha256=sha(log))
   if expected=='error':
    assert p.returncode!=0 and not stdout.strip() and 'No such file' in stderr,(name,stdout,stderr)
   else:
    assert p.returncode==0,(name,stdout,stderr)
    output=json.loads(stdout);assert output['managed_used']==0;e=output['result'];row['evidence']=output
    if expected=='exact':assert e=={'outcome':'exact_bytes'} and output['managed_peak']==0
    elif expected=='visual':
     assert e['outcome']=='complete' and e['supported'] is True and e['terminal']['kind']=='model'
     assert e==prior[control],name
     row['independent_visual_evidence_exact']=True
    else:
     assert e['outcome']=='inconclusive' and e['supported'] is None and e['projected']['attempts']==25
     assert e==prior[control],name
     row['independent_visual_evidence_exact']=True
   row['status']='passed';save();print(name,'passed',flush=True)
  verify_source();verify_pins()
  report.update(status='complete_bounded_frozen_exact_first_loftr_native_controls',invocations=len(cases),exact_controls=sum(c[5]=='exact' for c in cases),visual_controls=sum(c[5]=='visual' for c in cases),unknown_controls=sum(c[5]=='unknown' for c in cases),error_controls=sum(c[5]=='error' for c in cases),all_expected_outcomes_match=True)
 except Exception as e:report.update(status='failed_exact_first_loftr_native_controls',error=str(e));raise
 finally:save()
if __name__=='__main__':main()
