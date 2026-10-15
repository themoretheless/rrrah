"""Independently audit bounded native suffix controls and full acquisition pins; no full-review claim."""
from pathlib import Path
import json,hashlib,re,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/text85-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/google-research--deduplicate-text-datasets/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];row={**e,'sha256':h(p),'fully_read_this_turn':e['path'] in ['Cargo.toml','src/table.rs']}
 if row['fully_read_this_turn']:row['text_content']=b.decode()
 blobs.append(row)
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==15 and len(expected)==3
p=D/'text85-suffix-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-text85-suffix-controls-20261016.py')==d['producer_sha256'] and h(R/d['source'])==d['source_sha256'];assert len(d['runs'])==4
for name,want in d['artifact_pins'].items():assert h(R/name)==want
for row in d['runs']:
 name='checked' if row['checked_overflow'] else 'unchecked';a=W/f"suffix-controls/{name}-run{row['run']}.stdout";assert h(a)==row['stdout_sha256'];assert a.read_text().strip()=='TEXTS=3280 QUERIES=403440 MALFORMED_PANIC=1'
wrapper=(W/'suffix-controls/controls.rs').read_text();assert str(W/'source/src/table.rs') in wrapper and 'mod table;' in wrapper;assert 'text[*a as usize..].cmp' in wrapper and 'text[i..].starts_with(q)' in wrapper
compiler=subprocess.check_output(['rustc','+1.98.1','--version'],text=True).strip();assert '1.98.1' in compiler
acq=D/'text85-fetched-source-pins-20261016.json';emit(acq,{'repository':'google-research/deduplicate-text-datasets','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'scope':'All15blobs acquired andbyteverified, onlyCargo.toml/table.rs fullyread thisturn, main.rs lines1..260read. Otherfiles/remainingmain notyetreviewed; acquisition isnot fullsource reading.'})
emit(D/'text85-suffix-controls-audit-20261016.json',{'status':'verified_partial_source_complete_suffix_module_native_oracles','whole_repo_complete':False,'whole_implementation_complete':False,'repository':'google-research/deduplicate-text-datasets','commit':g['sha'],'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'auditor_sha256':h(Path(__file__)),'compiler_observed':compiler,'runs':4,'texts_per_run':3280,'queries_per_run':403440,'total_queries':1613760,'read_scope':'Whole940line src/table.rs andCargo.toml; main.rs1..260 only. No trackedbinaryassets intree, externalREADME/media notyetreviewed.','findings':d['findings'],'limits':d['limits']});print(json.dumps({'status':'verified','full_source':False,'total_queries':1613760,'acquired_blobs':15}))
