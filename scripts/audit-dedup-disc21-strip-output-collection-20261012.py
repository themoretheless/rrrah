#!/usr/bin/env python3
"""Compare full native outcomes and RSS to the qualified original control."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-disc21-strip-output-collection-audit-20261012.json';assert not out.exists();rp=D/'dedup-disc21-strip-output-collection-20261012.json';r=json.loads(rp.read_text());assert r['status']=='complete_bidirectional_collection_control'and r['build_returncode']==r['returncode']==0;assert all(sha(p)==v for p,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 mp=D/'dedup-strip-output-native-snapshot-20261012.json';m=json.loads(mp.read_text());snapshot=next(Path(p).parents[2]for p in r['pins']if p.endswith('/crates/rrrah-dedup/Cargo.toml')and 'strip-output-collection-source'in p);assert all(sha(snapshot/k)==v for k,v in m['file_hashes'].items())
 bp=D/'dedup-disc21-bidirectional-exposure-collection-20261012.json';ap=D/'dedup-disc21-bidirectional-exposure-collection-audit-20261012.json';b=json.loads(bp.read_text());a=json.loads(ap.read_text());assert a['direction']=='reverse_support_after_forward_miss'and all(sha(p)==v for p,v in a['pins'].items())
 v=json.loads(r['stdout']);old=json.loads(b['stdout']);assert v['evidence']==old['evidence'];assert v['managed_used']==0 and v['managed_report_used']==old['managed_report_used']
 stage=D/'dedup-loftr-strip-output-memory-audit-20261012.json';s=json.loads(stage.read_text());assert s['unit_tests']==7 and all(sha(p)==v for p,v in s['pins'].items());unit=D/'dedup-loftr-strip-output-candidate-20261012.json';u=json.loads(unit.read_text());assert u['tests']==7 and all(sha(p)==v for p,v in u['pins'].items());assert all(sha(Path(u['candidate'])/k)==v for k,v in u['source_file_hashes'].items())
 full=D/'dedup-loftr-strip-output-all-units-verification-20261012.json';f=json.loads(full.read_text());assert f['tests']==49 and all(sha(p)==v for p,v in f['pins'].items())
 rss=lambda x:int(re.search(r'(\d+)\s+maximum resident set size',x['stderr'])[1]);before,after=rss(b),rss(r)
 out.write_text(json.dumps({'status':'verified_full_cpu_strip_output_outcome_parity_and_rss_measurement','original_rss_bytes':before,'candidate_rss_bytes':after,'rss_reduction_bytes':before-after,'rss_improved':after<before,'pins':{str(p):sha(p)for p in [rp,bp,ap,mp,stage,unit,full,Path(__file__)]},'scope':'Exact full primary/recovery/geometry/pixel/exposure/transform traces for original-weight reversed-ID dark-positive CPU collection, released report credit, and separate-process whole-control RSS. Also49nativeunit tests including7backbone tests and synthetic stage measurement. No root adoption, totalRSS cap, full corpus/formats/platforms/heldout/full500 completion.'},indent=2)+'\n');print('Full outcome parity; RSS',before,after)
if __name__=='__main__':main()
