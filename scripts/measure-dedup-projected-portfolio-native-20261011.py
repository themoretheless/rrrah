#!/usr/bin/env python3
"""Frozen native/projected pair dispatch, unknowns and complete collection controls."""
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-projected-portfolio-snapshot-20261011.json'
FIXTURES=D/'dedup-projected-loftr-fixtures-20261011.json'
LEGACY=D/'dedup-projected-tiles-native-20261011.json'
OUT=D/'dedup-projected-portfolio-native-20261011.json'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
ARCHIVE=BASE/'projected-portfolio-qualified-20261011'
TORCH=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib')
WEIGHTS=BASE/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot']);f=json.loads(FIXTURES.read_text());work=Path(f['work'])
 old={r['case']:r['evidence'] for r in json.loads(LEGACY.read_text())['results']}
 def verify_source():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify_source()
 source=ROOT/'target/debug/projected_portfolio_files';binary=ARCHIVE/source.name;digest=sha(source)
 shutil.copy2(source,binary);assert sha(source)==sha(binary)==digest
 assert m['snapshot'].encode() in binary.read_bytes()
 base=snapshot/'crates/rrrah-dedup/tests/fixtures/rotation/base.png'
 pins={str(p):sha(p) for p in [MANIFEST,FIXTURES,LEGACY,Path(__file__),binary,WEIGHTS,base,
  D/'dedup-projected-portfolio-native-build-20261011.log',D/'dedup-loftr-native-20261010-runtime-pins.json']}
 pins.update(f['fixture_pins']);pins.update(f['source_pins'])
 runtime=json.loads((D/'dedup-loftr-native-20261010-runtime-pins.json').read_text())
 pins.update({p:h for p,h in runtime['pins'].items() if str(TORCH) in p});assert sum(str(TORCH) in p for p in pins)==7
 def verify_pins():assert all(sha(p)==h for p,h in pins.items())
 cases=[]
 for order in ['native-first','model-first']:
  for case,paths in [('alpha_copy',['alpha.qoi','alpha-copy.qoi']),('alpha_collision',['alpha.qoi','alpha-collision.qoi'])]:
   cases.append((case+'_'+order,order,[work/p for p in paths],case))
 cases += [('hdr_copy_native-first','native-first',[work/'hdr.pfm',work/'hdr-copy.pfm'],'hdr_copy'),
           ('hdr_collision_native-first','native-first',[work/'hdr.pfm',work/'hdr-collision.pfm'],'hdr_collision'),
           ('sdr_native_early','native-first',[base,base],None),
           ('alpha_collection','native-first',[work/'alpha.qoi',work/'alpha-copy.qoi',work/'alpha-collision.qoi'],'collection')]
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'source_manifest':str(MANIFEST),
  'results':[],'pair_invocations':7,'collection_invocations':1,'total_pair_records':10,
  'scope':__doc__}
 def save():
  t=OUT.with_suffix('.json.tmp');t.write_text(json.dumps(report,indent=2)+'\n');t.replace(OUT)
 def parity(e,case):
  prior=old[case]
  if e['outcome']=='complete':
   assert e['terminal']['kind']=='model'
   p=e['terminal']['evidence']
   return p['attempts']==prior['attempts'] and p['pixel_work']==prior['pixel_work'] and p['prepared_inputs']==prior['prepared_inputs']
  p=e['projected'];last=prior['attempts'][-1]
  return p['attempts']==len(prior['attempts']) and p['pixel_work']==prior['pixel_work'] and all(p[k]==last[k] for k in ['model_digest','raw_proposals','selected_points'])
 save()
 try:
  for name,order,paths,control in cases:
   verify_pins()
   command=['/usr/bin/time','-l','/usr/bin/env',f'DYLD_LIBRARY_PATH={TORCH}',str(binary),str(WEIGHTS),order,*map(str,paths)]
   row={'case':name,'status':'running','command':command};report['results'].append(row);save();start=time.monotonic()
   p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
   stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
   if p.returncode!=0:row['status']='execution_failed'
   else:
    try:
     output=json.loads(stdout);assert output['managed_used']==0;e=output['result'];row['evidence']=output
     if control=='collection':
      assert e['file_ids']==[0,1,2]
      pairs=e['pairs'];assert [(x['left'],x['right']) for x in pairs]==[(0,1),(0,2),(1,2)]
      assert [x['evidence']['supported'] for x in pairs]==[True,None,None]
      assert [x['evidence']['outcome'] for x in pairs]==['complete','inconclusive','inconclusive']
      assert all(x['evidence']['reason']=='PixelDomain(GradientOpacity)' and x['evidence']['projected']['attempts']==25 for x in pairs[1:])
      row['independent_projection_evidence_exact']=all(parity(x['evidence'],case) for x,case in zip(pairs,['alpha_copy','alpha_collision','alpha_collision']))
     elif control is None:
      assert e['outcome']=='complete' and e['supported'] is True and e['terminal']['kind']=='native' and e['attempted_stages']==1
      row['independent_projection_evidence_exact']=True
     elif control.endswith('copy'):
      assert e['outcome']=='complete' and e['supported'] is True and e['terminal']['kind']=='model'
      assert e['attempted_stages']==(2 if order=='native-first' else 1)
      if control=='alpha_copy' and order=='native-first':assert 'GradientOpacity' in e['first_miss']
      row['independent_projection_evidence_exact']=parity(e,control)
     else:
      assert e['supported'] is not True
      if control=='alpha_collision':
       assert e['outcome']=='inconclusive' and e['reason']=='PixelDomain(GradientOpacity)' and e['projected']['attempts']==25
      row['independent_projection_evidence_exact']=parity(e,control)
     row['status']='passed';row['expected_outcome_matches']=True
    except Exception as error:row.update(status='outcome_failed',error=str(error),expected_outcome_matches=False)
   verify_pins();save();print(name,row['status'],row.get('independent_projection_evidence_exact'),flush=True)
  verify_source();verify_pins()
  report.update(status='complete_bounded_eight_native_projected_portfolio_controls_execution',
   all_expected_outcomes_match=all(r.get('expected_outcome_matches',False) for r in report['results']),
   all_independent_projection_evidence_exact=all(r.get('independent_projection_evidence_exact',False) for r in report['results']),
   limits=['Seven pair invocations plus one three-file collection are bounded controls, not broad recall or negative/burst precision',
    'Unknown collection outcomes remain distinct from negatives; no transitive file identity',
    'No total Torch tensor/workspace/RSS bound; shared-host timings are not performance benchmarks',
    'No Linux/Windows Torch qualification or indexed collection scalability'])
 except Exception as error:report.update(status='failed_projected_portfolio_native_controls',error=str(error));raise
 finally:save()
if __name__=='__main__':main()
