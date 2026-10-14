#!/usr/bin/env python3
"""Original module on synthetic exports only; never inspect a real vault."""
from pathlib import Path
import importlib.util,json,hashlib,copy,subprocess,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/biplobice--bitwarden-deduplicator';reg=json.loads((D/'dedup-500/reviews/178.json').read_text());tree=json.loads((E/'tree.json').read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();items=[v for k in ['available_source_evidence','available_readme_license_evidence']for v in reg[k]];blobs={v['path']:v for v in tree['tree']if v['type']=='blob'};assert len(items)==len(blobs)==3 and not tree['truncated'];files=[]
for v in items:
 p=E/v['local'];b=p.read_bytes();assert sha(p)==v['sha256']and len(b)==blobs[v['path']]['size']and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==blobs[v['path']]['sha'];files.append(dict(path=v['path'],local=str(p.relative_to(R)),sha256=sha(p),git_blob_sha1=blobs[v['path']]['sha'],bytes=len(b)))
module=E/'02-deduplicate_bitwarden.py';spec=importlib.util.spec_from_file_location('retained_bitwarden178',module);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);K=D/'dedup-bitwarden178-controls-20261014';K.mkdir(exist_ok=False);rows=[]
def login(i,user='synthetic-user',uris=('example.invalid',),password='synthetic-password'):
 return dict(id=i,name='synthetic-'+i,type=1,login=dict(username=user,password=password,uris=[dict(uri=u)for u in uris]))
def run(label,records=(),folders=(),expected=1,validate=None,error=None):
 p=K/(label+'-input.json');o=K/(label+'-output.json');p.write_text(json.dumps(dict(folders=folders,items=records)));before=sha(p)
 try:
  result=m.deduplicate_bitwarden_export(str(p),str(o),quiet=True)
 except BaseException as e:
  assert error and isinstance(e,error),(label,type(e));assert not o.exists();rows.append(dict(label=label,error=type(e).__name__,input_sha256=sha(p)));return
 assert error is None and sha(p)==before;v=json.loads(o.read_text());assert len(v['items'])==expected,(label,v)
 if validate:validate(v)
 rows.append(dict(label=label,input_sha256=sha(p),output_sha256=sha(o),retained_items=len(v['items']),summary=result))
run('different_passwords_merge',[login('a',password='synthetic-one'),login('b',password='synthetic-two')])
run('different_notes_merge',[dict(id='a',name='note-a',type=2,notes='synthetic-a'),dict(id='b',name='note-b',type=2,notes='synthetic-b')])
run('different_cards_merge',[dict(id='a',type=3,card=dict(number='synthetic-card-a')),dict(id='b',type=3,card=dict(number='synthetic-card-b'))])
run('different_names_merge',[login('first-name'),login('second-name')])
run('public_suffix_collision',[login('a',uris=('one.co.uk',)),login('b',uris=('two.co.uk',))])
run('ip_suffix_collision',[login('a',uris=('10.1.2.3',)),login('b',uris=('192.9.2.3',))])
run('paths_ports_ignored',[login('a',uris=('https://example.invalid:111/a',)),login('b',uris=('https://example.invalid:222/b',))])
run('username_case_collapses',[login('a',user='SyntheticUser'),login('b',user='syntheticuser')])
run('delimiter_composite_collision',[login('a',user='u|x',uris=('example.invalid',)),login('b',user='u',uris=('x|example.invalid',))])
a=login('same',uris=('a.invalid','b.invalid'));run('identical_multi_uri_rows_retained',[a,copy.deepcopy(a)],expected=2)
a=login('a',user='one');a['folderId']='A';b=login('b',user='two');b['folderId']='B'
def dangling(v):assert [f['id']for f in v['folders']]==['A']and v['items'][1]['folderId']=='B'
run('duplicate_folder_id_not_remapped',[a,b],[dict(id='A',name='same'),dict(id='B',name='same')],expected=2,validate=dangling)
run('empty_folders_removed',[],[dict(id='A',name=''),dict(id='B',name='')],expected=0,validate=lambda v:None if not v['folders']else (_ for _ in ()).throw(AssertionError()))
a=login('a');a['login']['uris']=None;run('null_uris_untyped_failure',[a],error=TypeError)
a=login('a');a['login']['username']=42;run('numeric_username_untyped_failure',[a],error=AttributeError)
run('empty_export',[],expected=0)
p=K/'malformed-input.json';p.write_text('{');
try:m.deduplicate_bitwarden_export(str(p),str(K/'malformed-output.json'),quiet=True)
except SystemExit as e:assert e.code==1;rows.append(dict(label='malformed_json_systemexit',exitcode=e.code))
else:raise AssertionError('malformedJSONaccepted')
p=K/'cli-input.json';p.write_text(json.dumps(dict(items=[login('a'),login('b')])));before=sha(p);r=subprocess.run([sys.executable,str(module),str(p),'-q','-s',str(K/'cli-summary.md')],capture_output=True,text=True);assert r.returncode==0 and r.stdout==r.stderr==''and sha(p)==before;assert len(json.loads((K/'cli-input_deduplicated.json').read_text())['items'])==1;assert (K/'cli-summary.md').exists();rows.append(dict(label='real_cli_quiet_summary',returncode=r.returncode))
p=K/'inplace-input.json';p.write_text(json.dumps(dict(items=[login('a'),login('b')])));before=sha(p);m.deduplicate_bitwarden_export(str(p),str(p),quiet=True);assert sha(p)!=before and len(json.loads(p.read_text())['items'])==1;rows.append(dict(label='inplace_overwrite_accepted'))
assert len(rows)==18;O=D/'dedup-bitwarden178-controls-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified18synthetic_original_module_controls',repository=reg['repository'],commit=reg['commit'],files=files,rows=rows,controls_passed=18,producer_sha256=sha(__file__),artifact_pins={str(p.relative_to(R)):sha(p)for p in K.iterdir()},scope='All3capturedGitblobs fullyread and verified. Original252line module imported unmodified and originalCLI executed only on newlycreated syntheticJSON files. No uservault, realcredential, Bitwardenaccount/API, private file, source adoption, file/pixel identity or platform/resource qualification. No upstream tests in capturedtree; module semanticloss and duplicate misses are observations, not endorsement.'),indent=2)+'\n');print('18synthetic original module controls passed')
