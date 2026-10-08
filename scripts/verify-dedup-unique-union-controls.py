#!/usr/bin/env python3
"""Confirm final unique-union controls preserve the preceding native evidence."""
import argparse,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('report','old_controls','old_gradient_report','output'):p.add_argument(key,type=Path)
a=p.parse_args();paths=(a.report,a.old_controls,a.old_gradient_report);raw={v:v.read_bytes() for v in paths};new,old,gradient=[json.loads(raw[v]) for v in paths]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert new['status']=='complete' and new['required_pairs']==len(new['results'])==3
assert [v['query_id'] for v in new['results']]==['200001.jpg','200102.jpg','200901.jpg']
assert old['status']=='complete' and old['required_pairs']==len(old['results'])==4
for report in (new,old,gradient):
 assert all(digest(path)==value for path,value in report['input_hashes'].items())
 assert all(digest(path)==value for path,value in report['image_hashes'].items())
refs={v['query_id']:v['evidence'] for v in old['results']}
refs.update({v['query_id']:v['evidence'] for v in gradient['results']})
changes=[]
for row in new['results']:
 qid=row['query_id'];e=row['evidence'];prior=refs[qid]
 assert {k:v for k,v in e.items() if k!='managed_peak'}=={k:v for k,v in prior.items() if k!='managed_peak'},qid
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
 changes.append({'query_id':qid,'old_managed_peak':prior['managed_peak'],'new_managed_peak':e['managed_peak']})
for path,data in raw.items():assert path.read_bytes()==data,'Report changed; retry audit.'
result={'status':'verified','verified_pairs':3,'all_native_fields_equal_except_peak':True,'report_hashes':{str(path):hashlib.sha256(data).hexdigest() for path,data in raw.items()},'managed_peaks':changes,'scope':'Three two-source native controls across frozen versions. No full-corpus performance/RSS, all-query recall/precision, full collection scale or full-library qualification.'}
a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
