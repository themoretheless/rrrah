"""Independently verify complete pinned source tree and selected native controls."""
from pathlib import Path,PurePosixPath
import hashlib,json
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/es113-20261016';D=R/'docs/research/dedup-500'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
tree=json.loads((D/'evidence/deric--es-dedupe/tree.json').read_text());commit=json.loads((W/'commit.json').read_text())
assert not tree['truncated'] and commit['sha']=='7abed1353a32cf72b763067b33e46fc62b85ddc2' and commit['tree']['sha']==tree['sha']
entries=tree['tree'];blobs=[e for e in entries if e['type']=='blob'];assert len(blobs)==22
pins=[]
for e in blobs:
 p=W/'source'/e['path'];b=p.read_bytes();b.decode('utf8');assert len(b)==e['size']
 assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==e['sha']
 pins.append({'path':str(p.relative_to(R)),'repository_path':e['path'],'git_blob':e['sha'],'sha256':h(p),'bytes':len(b),'read_scope':'complete UTF8 text read'})
trees=[]
for dirname in ['']+[e['path'] for e in entries if e['type']=='tree']:
 children=[e for e in entries if str(PurePosixPath(e['path']).parent).replace('.','',1)==dirname] if not dirname else [e for e in entries if str(PurePosixPath(e['path']).parent)==dirname]
 children.sort(key=lambda e:(PurePosixPath(e['path']).name+('/' if e['type']=='tree' else '')).encode())
 b=b''.join((e['mode'].lstrip('0')+' '+PurePosixPath(e['path']).name).encode()+b'\0'+bytes.fromhex(e['sha']) for e in children)
 got=hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest();expected=tree['sha'] if not dirname else next(e['sha'] for e in entries if e['path']==dirname)
 assert got==expected,(dirname,got,expected);trees.append({'path':dirname,'sha':got})
controls=D/'es113-boundary-controls-20261016.json';c=json.loads(controls.read_text());producer=R/'scripts/qualify-es113-boundaries-20261016.py'
assert c['producer_sha256']==h(producer) and c['cases_per_run']==22 and c['executions']==44 and c['rows'][0]==c['rows'][1]
assert all(row['observed']==row['expected'] for row in c['rows'][0])
for dep in c['dependencies']:assert h(R/dep['path'])==dep['sha256']
p=D/'es113-complete-source-review-20261016.json';assert not p.exists()
p.write_text(json.dumps({'status':'verified_complete_pinned_source_and_selected_native_boundaries','repository':'deric/es-dedupe','commit':commit['sha'],'whole_repo_complete':True,'whole_implementation_complete':True,'blobs':len(blobs),'canonical_trees':len(trees),'pins':pins,'trees':trees,'commit_record':str((W/'commit.json').relative_to(R)),'commit_record_sha256':h(W/'commit.json'),'auditor_sha256':h(Path(__file__)),'controls':str(controls.relative_to(R)),'controls_sha256':h(controls),'native_cases':22,'native_executions':44,'source_scope':'All22tracked UTF8 blobs read completely including4integrationtestfiles, CLI/core/utils/cmd/packaging/Docker/CI/build/docs/completeApache2license. No tracked binary media. Two external dynamic microbadger status badges not retrieved; no external site review claimed.','upstream_tests_run':False,'upstream_test_scope':'All8original integrationtest methods across4files read, not executed against an Elasticsearch service.','findings':['Field tuples concatenated without framing: (ab,c)/(a,bc) shareMD5 before anycryptographiccollision. Singlefield str() mergesinteger1/string1 andNone/stringNone.','Exactonewindow processeszero queries; exacttwowindows processesonlyfirst. Partialtail11minutes processes3queries; short2minutes1query. Emptyinterval0minutes still issuesemptyrange query.','Zero/negativewindow accepted by converter; loop notexecuted for these cases.','Datetimeoffset+05 formatsunchanged walltimeasZ anddropsmicroseconds, noUTCconversion. CLI rejectsfractional/Zdates usedinREADMEexample.','Deleteiterator preservesfirstscanorder, notearliest/latestdate; no routingorcontent proof. Actualrecordedoperationsonly, noESdelete.','Sequential/parallel wrappers count successfulshardreplicas, notdoccount; closedgeneratorerror swallowed even failfaststyle intent.','No-timestamp queryreturnsNone; existingporthost gainssecondport.','CLI echoesallarguments including literalpassword; parserreuse repeats add_argument andraisesArgumentError.','Packagingstatic: adjacentstringliterals combine elasticsearch>5.0 andpsutil intoone malformed requirement; python-benedict omitted. DockerfinalCMD points/usr/local/bin althoughvenventrypoint installed/opt/venv/bin. No actualbuild/packageexecutionclaim.','Windowpartition cannotfind duplicates acrossdistinctwindows, wholeinmemorydict unbounded; --all/prefix/indexexclude accepted butrunonlyprocessesexplicitindex. Staticcontractfindings.'],'decision':'Reject as image/fileidentity implementation. Retain regression lessons: typed framed keys, independent byteverification, explicitrepresentativepolicy, fullrangecoverage, scopedresourcebudgets andfailureaccounting. No sourcecode copied into production.','limits':c['limits'],'license':'CompleteApache2.0LICENSE read; no productioncodecopied.'},indent=2)+'\n')
print(json.dumps({'status':'verified','blobs':len(blobs),'trees':len(trees),'native_executions':44}))
