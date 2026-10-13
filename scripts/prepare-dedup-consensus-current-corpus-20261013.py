#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,math,collections
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();native_path=D/'dedup-consensus-native-plan-20261013.json';native=json.loads(native_path.read_text());S=Path(native['source']);assert all(sha(S/k)==h for k,h in native['source_sha256'].items());positive_path=D/'dedup-current-native-bank-all-positives-20261012.json';positive=json.loads(positive_path.read_text());assert positive['status']=='complete_current_fixed458_execution' and positive['completed']==458;negative_path=D/'dedup-copydays-nearest-wrong-native-resume-prefix-0140-20261013.json';negative=json.loads(negative_path.read_text());semantic_path=D/'dedup-semantic-category-negatives-plan-20261013.json';semantic=json.loads(semantic_path.read_text());groups=collections.OrderedDict()
for c in positive['plan']:groups.setdefault((c['query'],c['original']),[]).append(c)
assert len(groups)==229 and all(len(v)==2 and {(x['left'],x['right']) for x in v}=={key,key[::-1]} for key,v in groups.items());positives=[dict(left=k[0],right=k[1],label='copydays_positive',expected='duplicate',groundtruth='publisher_origin',origin=k[1]) for k in groups];groups=collections.OrderedDict()
for c in negative['plan']:groups.setdefault((c['query'],c['wrong_reference']),[]).append(c)
assert len(groups)==229 and all(len(v)==2 for v in groups.values());negatives=[]
for (left,right),items in groups.items():
 assert Path(left).stem[:4]!=Path(right).stem[:4];negatives.append(dict(left=left,right=right,label='copydays_nearest_wrong',expected='different',groundtruth='publisher_excluded_wrong_origin',origin=items[0]['origin'],priority_rank=items[0]['priority_rank']))
groups=collections.OrderedDict()
for c in semantic['plan']:groups.setdefault(tuple(sorted([c['left'],c['right']])),[]).append(c)
assert len(groups)==4 and all(len(v)==2 for v in groups.values())
for rows in groups.values():
 c=rows[0];assert c['published_label']=='no_reference_match';negatives.append(dict(left=c['left'],right=c['right'],label='semantic_negative',expected='different',groundtruth=c['published_label'],category_reason=c['manual_category_reason']))
assert len(negatives)==233;controls_path=D/'dedup-consensus-native-v2-execution-prefix-0005-20261013.json';controls=json.loads(controls_path.read_text());controls_by_pair={}
for r in controls['results']:
 case=json.loads(Path(r['report']).read_text());controls_by_pair.setdefault(tuple(case['command'][-8:-6]),[]).append((r,case))
exe=Path('/Users/themoretheless/.codex/tmp/rrrah-consensus-native-v2-20261013/probe');weights=Path(native['cases'][0]['arguments'][2]);shared_paths=[native_path,positive_path,negative_path,semantic_path,controls_path,D/'dedup-consensus-native-focused-audit-20261013.json',D/'dedup-consensus-native-learned-audit-20261013.json',D/'dedup-consensus-complete-decode-audit-20261013.json',Path(__file__),exe,weights];base_pins={str(p):sha(p) for p in shared_paths};base_pins.update({k:h for k,h in native['pins'].items() if k.endswith('.dylib')});assert all(sha(k)==h for k,h in base_pins.items())
for stratum,cases in [('positive',positives),('negative',negatives)]:
 pins=dict(base_pins)
 for i,c in enumerate(cases):
  c['index']=i
  with Image.open(c['left']) as a,Image.open(c['right']) as b:ad=list(a.size);bd=list(b.size)
  ft=2*math.hypot(*ad)/math.hypot(*bd);rt=2*math.hypot(*bd)/math.hypot(*ad);c.update(source_dimensions=ad,target_dimensions=bd,source_tolerance=ft,reverse_source_tolerance=rt,arguments=[c['left'],c['right'],str(weights),'640',repr(ft),'1000000',repr(rt),'false'])
  for path in [c['left'],c['right']]:
   pins[path]=sha(path);old_pin=positive['pins'].get(path,negative['pins'].get(path,semantic['pins'].get(path)));assert old_pin==pins[path],path
  for r,prior in controls_by_pair.get((c['left'],c['right']),[]):
   if prior['returncode']==0 and prior['command'][-8:]==c['arguments'] and sha(prior['command'][-9])==sha(exe):c['reused_control']=dict(report=r['report'],sha256=r['sha256']);pins[r['report']]=r['sha256'];break
 P=D/f'dedup-consensus-current-{stratum}-plan-20261013.json';assert not P.exists();P.write_text(json.dumps(dict(status='fixed_before_corpus_execution',stratum=stratum,required=len(cases),source=str(S),source_manifest=str(native_path),source_manifest_sha256=sha(native_path),executable=str(exe),pins=pins,cases=cases,scope='Current frozen native atomic consensus; same640input/CPU2/seed173once/1m opposite/25modelbanks/radii2,6/linear-sRGB interpretation/512MiBmanaged policy for all labels. Both source tolerances use2*sourceDiagonal/targetDiagonal independently. Pair-quality campaign, not collection atomicity/default adoption, whole-image identity, hard RSS/resource/platform/format or full500source proof. Reuse only exact8argument and executable-hash matches from immutable actual atomic controls.'),indent=2)+'\n');print(stratum,len(cases),'reused',sum('reused_control'in c for c in cases))
