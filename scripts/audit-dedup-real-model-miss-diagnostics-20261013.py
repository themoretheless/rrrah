"""Verify actual fixed LoFTR execution, unchanged decisions and complete source/runtime pins."""
import pathlib,json,hashlib,re
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-real-model-miss-diagnostics-20261013.json';d=json.loads(P.read_text());assert d['status']=='complete_pending_audit' and d['build_returncode']==0
T=pathlib.Path('/Users/themoretheless/.codex/tmp');S=pathlib.Path(d['source']);H=pathlib.Path(d['harness'])
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
assert all(sha(p)==h for p,h in d['pins'].items());prep=D/'dedup-real-model-miss-preparation-20261013.json';m=json.loads(prep.read_text());assert all(sha(p)==h for p,h in m['pins'].items());assert all(sha(S/p)==h for p,h in m['source_hashes'].items())
O=T/'rrrah-current-native-bank-all-positives-harness-20261012';old_source=T/'rrrah-dedup-current-decode-source-20261012';assert (H/'Cargo.toml').read_text()==(O/'Cargo.toml').read_text().replace(str(old_source),str(S));assert sha(H/'Cargo.lock')==sha(O/'Cargo.lock');assert sha(H/'src/native_policy.rs')==sha(O/'src/native_policy.rs')
caller=(H/'src/bin/native_bank_miss_diagnostics_20261013.rs').read_text();begin=caller.index('"model_search":p.evidence.model_search');end=caller.index('"native_miss":',begin);assert caller[:begin]+caller[end:]==(O/'src/bin/native_bank_current_all_positives.rs').read_text()
rows=[];pins={str(P):sha(P),str(prep):sha(prep),str(pathlib.Path(__file__)):sha(__file__)}
assert len(d['plan'])==len(d['results'])==2
for item,row in zip(d['plan'],d['results']):
 assert item['kind']==row['kind'];path=pathlib.Path(row['report']);assert sha(path)==row['sha256'];c=json.loads(path.read_text());assert c['returncode']==0
 expected=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),d['binary'],item['left'],item['right'],item['weights'],'640',str(item['tolerance']),'1000000'];assert c['command']==expected
 value=json.loads(c['stdout']);old=json.loads(json.loads(pathlib.Path(item['baseline']).read_text())['stdout']);assert value['managed_used']==0 and value['files']==[2,9] and len(value['pairs'])==1
 diagnostic=value['pairs'][0].pop('model_search');support=value['pairs'][0]['support']
 for field in ['managed_peak','managed_retained_before_drop']:value.pop(field);old.pop(field)
 assert value==old,(item['kind'],value,old)
 if item['kind']=='fixed_negative_index0':
  assert support is None and diagnostic is not None;assert diagnostic['attempts']==25;assert diagnostic['pixel_attempts']==diagnostic['pixel_geometry_refusals']+diagnostic['pixel_rejections'];assert not diagnostic['confirmed_transform']
 else:assert support is True
 rss=int(re.search(r'(\d+)\s+maximum resident set size',c['stderr'])[1]);rows.append(dict(kind=item['kind'],support=support,diagnostics=diagnostic,rss_bytes=rss,elapsed_seconds=c['elapsed_seconds'],decisions_and_other_json_equal_baseline=True));pins[str(path)]=sha(path)
out={'status':'verified_fixed_real_loftr_diagnostics_controls','executed':2,'source_files_verified':len(m['source_hashes']),'torch_dylibs_verified':len([p for p in m['pins'] if p.endswith('.dylib')]),'rows':rows,'pins':pins,'scope':'One fixed published-label negative and one fixed publisher positive with same original policy/runtime. Complete diagnostic addition and unchanged decisions/JSON apart from declared managed size/peak fields. No general negative precision, performance improvement, hardRSS bound or full458 qualification.'}
(D/'dedup-real-model-miss-diagnostics-audit-20261013.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({'status':out['status'],'rows':rows},indent=2))
