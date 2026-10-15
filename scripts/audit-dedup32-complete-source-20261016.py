"""Verify all source/tree/media bytes and scoped native/wholeCLI evidence independently."""
from pathlib import Path,PurePosixPath
import base64,hashlib,json
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dedup32-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();d=json.loads((W/'github-tree.json').read_text());tree=d['tree']['tree'];assert not d['tree']['truncated'] and len(tree)==35;files=[]
for x in tree:
 if x['type']!='blob':continue
 p=S/x['path'];b=p.read_bytes();assert len(b)==x['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['sha'];row={'path':x['path'],'git_blob':x['sha'],'bytes':len(b),'sha256':h(p)}
 if p.suffix=='.png':row.update(encoding='base64',content=base64.b64encode(b).decode(),review='Entire originalbanner visually viewed')
 else:row.update(encoding='utf8',content=b.decode(),review='Entiretextread')
 files.append(row)
assert len(files)==31;verified=[]
for directory,expected in [('',d['commit']['tree']['sha'])]+[(x['path'],x['sha']) for x in tree if x['type']=='tree']:
 children=[x for x in tree if str(PurePosixPath(x['path']).parent)==(directory or '.')];children.sort(key=lambda x:(PurePosixPath(x['path']).name+('/' if x['type']=='tree' else '')).encode());body=b''
 for x in children:body+=('40000' if x['type']=='tree' else x['mode']).encode()+b' '+PurePosixPath(x['path']).name.encode()+b'\0'+bytes.fromhex(x['sha'])
 actual=hashlib.sha1(b'tree '+str(len(body)).encode()+b'\0'+body).hexdigest();assert actual==expected;verified.append({'directory':directory,'sha':actual})
assert len(verified)==5;reports=[D/'dedup32-native-large-clone-controls-20261016.json',D/'dedup32-whole-cli-controls-20261016.json'];producers=['qualify-dedup32-native-large-clone-20261016.py','qualify-dedup32-whole-cli-20261016.py']
for report,producer,count in zip(reports,producers,[20,10]):
 c=json.loads(report.read_text());assert h(R/'scripts'/producer)==c['producer_sha256'] and len(c['rows'])==count;seen=set()
 for name,digest in c['source_pins'].items():assert h(R/name)==digest
 for row in c['rows']:
  key=(row['repeat'],row['case']);assert key not in seen;seen.add(key);p=R/row['log'];assert h(p)==row['log_sha256']
  if row['case'] in ['2m','4g','4g64k']:
   parts=next(x for x in p.read_text().splitlines() if x.startswith('HASH ')).split();assert parts[1:4]==['0','0','4'] and parts[4:]==row['native_digests'];a=W/'native-controls'/f"run{row['repeat']}"/row['case']/'a';b=a.with_name('b');assert a.stat().st_size==b.stat().st_size==row['size']
   for path,expected_byte in [(a,b'X'),(b,b'Y')]:
    with path.open('rb') as f:f.seek(row['changed_offset']);assert f.read(1)==expected_byte
   if row['size']>=2**32:
    with a.open('rb') as f:prefix=f.read(row['prefix_length_used_by_32bit_API'])
    assert row['native_digests'][0]==row['native_digests'][1]==row['prefix_sha256_oracle']==hashlib.sha256(prefix).hexdigest()
   else:assert row['native_digests'][0]!=row['native_digests'][1]
  if row['case']=='4g64k-distinct-actual-wrong-clone':
   assert row['before_distinct_bytes']==['58','59'] and row['after_equal_bytes'][0]==row['after_equal_bytes'][1];assert 'duplicates found: 1' in p.read_text() and 'cloned to' in p.read_text()
   case=W/'whole-cli'/f"run{row['repeat']}-4g64k-live"
   for name,expected_byte in zip(['a','b'],row['after_equal_bytes']):
    with (case/name).open('rb') as f:f.seek(row['different_byte_offset']);assert f.read(1).hex()==expected_byte
acquisition=D/'dedup32-fetched-source-pins-20261016.json';assert not acquisition.exists();acquisition.write_text(json.dumps({'repository':'ttkb-oss/dedup','commit':d['commit'],'verified_trees':verified,'files':files},indent=2)+'\n')
out=D/'dedup32-complete-source-review-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_complete_source_media_review_and_actual_native_cli_largefile_failure','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'ttkb-oss/dedup','commit':'a8e82b5aa7c14fbad203b8018cb440dc0560917a','blobs':31,'canonical_trees':5,'acquisition':str(acquisition.relative_to(R)),'acquisition_sha256':h(acquisition),'auditor_sha256':h(__file__),'native_reports':{str(x.relative_to(R)):h(x) for x in reports},'upstream_tests_run':False,'upstream_test_scope':'Alltestsource/Makefile read; Check dependency absent frompkgconfig, originalsuite notrun. NoHFSimage mounts/globalVolumes/ACL/immutability fixtures executed. OriginalwholeCLI andselectednativecontrols instead.','license':'BSD2clause LICENSE and every file notice read; no productioncodecopy','findings':[{'qualification':'ActualCommonCrypto native module10controls twice andactualwholeCLI5controls twice','finding':'size_t filelength passed to CC_SHA256_Update CC_LONG32bit truncates modulo4GiB. Native4GiB digestempty and4GiB+64KiB digestfirst64KiB despite differingmiddlebyte. ActualwholeCLI misclassifiesdistinctfiles and overwritesonewithclone on owned4GiB+64KiBsparsefixtures twice. 2MiBdistinctcontrol correct.'},{'qualification':'Originalnativeclone module twice','finding':'Successfultinyclone preservesdestinationmode andCOW, missingclone-source/stagingcollision/zeroclone preservetarget. DirecthardlinkAPI missing-source unlinks target before failure; notwholeCLI disappearing-source reproduction.'},{'qualification':'Static/documented only','finding':'README explicitly discloses mutable-source clone races and no filelocking. No concurrent file mutation exploited inthisrun.'},{'qualification':'Static only','finding':'Duplicate grouping uses SHA256digest without byte-equality fallback; duplicate tree keyed only digest, notdevice. Cross-volume and cryptographiccollision consequences untested.'},{'qualification':'Static only','finding':'Errorpaths leak openFILE/fd/mmap resources; may_share_blocks usesORmask ratherthanAND; alistzero-capacity growth cannotadvance; no runtimequalification ofthese branches.'},{'qualification':'Static only','finding':'CLIthread_count uint8 andatoi allowwrapping/invalidtext; stagingprefixskip inspects fullpath prefixratherthan basename; no boundary/runtimeconcurrency qualification.'}],'decision':'Reference staged clone/metadata/source-selection concepts only; do notadopt truncatedhash or destructiveerrorhandling. Correctness mustprecede speed.','limits':['Full31blobsource/media review declaration backed bypins, notinferred fromfetch','ActualMacARM/APFS nativeAPI andwholeCLI t0controls; noLinux/FreeBSD/Windows/nativeparallel/strictoriginalmake/ASAN/completeupstreamsuite qualification','Sparse logicalfiles with independentlychecked differingbytes/truncatedprefix, not full4GiBphysicalIO/hardRSS/performance evidence','Allfiles mutated belonged toownedfixtures; no userfiles or volumes touched, no productionadoption/speedup claim']},indent=2)+'\n');print('Verified31blobs/5trees/fullmedia; native20andwholeCLI10controlprocesses pinned;128th fullreview ready')
