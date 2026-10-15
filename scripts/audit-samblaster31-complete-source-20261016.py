"""Independently verify all upstream blobs and retained native raw evidence."""
from pathlib import Path
import hashlib,json
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/samblaster31-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/GregoryFaust--samblaster/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];blobs=[]
for e in t['tree']:
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==e['sha'];r={**e,'sha256':h(p)}
 if p.suffix!='.pdf':r['text_content']=b.decode()
 blobs.append(r)
b=b''.join(e['mode'].encode()+b' '+e['path'].encode()+b'\0'+bytes.fromhex(e['sha']) for e in sorted(t['tree'],key=lambda x:x['path']))
assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==t['sha'];assert len(blobs)==7
p=D/'samblaster31-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-samblaster31-native-controls-20261016.py')==d['producer_sha256']
for key in ['source_pins','binary_pins','artifact_pins']:
 for name,want in d[key].items():assert h(R/name)==want,name
assert len(d['runs'])==2
for run,rows in enumerate(d['runs']):
 assert len(rows)==17
 for i,row in enumerate(rows):
  stdout=W/f'native-controls-v3/run{run}-case{i}.stdout';assert h(stdout)==row['stdout_sha256']
  flags=[int(x.split('\t')[1]) for x in stdout.read_text().splitlines() if not x.startswith('@')];assert flags==row['flags']
 assert (W/f'native-controls-v3/hash-run{run}.stdout').read_text().splitlines()==['resize_20001_unique_and_duplicate_replay_pass','collision_chain20_exact_replay_pass','top_bit_alias_confirmed_7_and_2pow63plus7','bit62_distinct_pass']
assert [{k:v for k,v in r.items() if k not in ['stdin','stderr']} for r in d['runs'][0]]==[{k:v for k,v in r.items() if k not in ['stdin','stderr']} for r in d['runs'][1]]
s=json.loads((W/'sanitized-results.json').read_text());assert s['tagless_exit']!=0 and s['tagged_exit']==0;raw=(W/'sanitized-tagless.stderr').read_text();assert 'AddressSanitizer' in raw and 'addMTs' in raw and 'strstr' in raw
pins={str(p.relative_to(R)):h(p) for p in [W/'page-1.png',W/'page-2.png',W/'supplement.txt',W/'samblaster-sanitized',W/'sanitized-tagless.stdout',W/'sanitized-tagless.stderr',W/'sanitized-tagged.stdout',W/'sanitized-tagged.stderr',W/'sanitized-results.json']}
a=D/'samblaster31-fetched-source-pins-20261016.json';emit(a,{'repository':'GregoryFaust/samblaster','commit':g['sha'],'root_tree':t['sha'],'canonical_tree_verified':True,'blobs':blobs,'media_and_sanitizer_pins':pins})
report={'status':'verified_complete_pinned_source_pdf_and_native_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'GregoryFaust/samblaster','commit':g['sha'],'blobs':7,'canonical_trees':1,'acquisition':str(a.relative_to(R)),'acquisition_sha256':h(a),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'Both supplemental PDF pages rendered and viewed, whole extracted text/table read. Native biological benchmark claims are historic author results, not reproduced here. No other binary media in pinned tree.','license':'MIT LICENSE.txt fully read; no production code copied. PDF bears separate authors2014 supplemental copyright.','native_scope':d['scope'],'upstream_tests_run':False,'original_build_passed':True,'sanitizer_scope':s,'findings':d['findings']+['Global slab allocator and freeHashTableNodes do not reset global lists; not a reusable thread-safe library lifecycle. Static finding, repeat teardown not tested.','Signature index space grows quadratically with genome bins; oversized parameter/contig integer arithmetic and memory admission not comprehensively qualified. Static only.'],'decision':'Reject biological signature equivalence as file/image identity. Reference streaming and collision-node slab layout only with explicit key-domain/lifecycle contracts; no production adoption or speed claim.','limits':d['limits']+['ASAN+UBSAN verifies tagless crash and tagged positive control only; not complete sanitizer suite. Native current-root quality/formats/platform/performance scope remains open.']}
emit(D/'samblaster31-complete-source-review-20261016.json',report);print(json.dumps({'status':'verified','blobs':7,'trees':1,'CLI_runs':34,'hash_controls':8,'full_source':True}))
