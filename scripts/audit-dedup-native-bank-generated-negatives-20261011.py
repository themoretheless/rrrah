#!/usr/bin/env python3
import hashlib,json,math,re,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def pixels(p):
 b=Path(p).read_bytes();assert b[:2]==b'BM' and struct.unpack_from('<I',b,10)[0]==54
 assert struct.unpack_from('<iiHH',b,18)==(128,128,1,24) and len(b)==54+128*128*3
 rgb=b[54:];assert all(rgb[i]==rgb[i+1]==rgb[i+2] for i in range(0,len(rgb),3));return list(rgb[::3])
def main():
 report=D/'dedup-native-bank-generated-negatives-20261011.json';r=json.loads(report.read_text());assert r['status']=='complete_bounded_generated_negative_controls' and r['required']==2
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());mp=D/'dedup-native-bank-outcome-native-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 ca=D/'dedup-native-bank-outcome-native-compile-audit-20261011.json';c=json.loads(ca.read_text());assert c['status']=='verified_146_test_core_cpu_collection_wrapper_compile' and all(sha(k)==v for k,v in c['pins'].items())
 assert [x['case'] for x in r['results']]==['flat_brightness','independent_seeded_noise'];rows=[]
 for row in r['results']:
  assert row['status']=='complete_no_support' and row['returncode']==0 and row['command'][-3:]==['640','0.5','1000000'];assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256']
  v=json.loads(row['stdout']);assert v['files']==[2,9] and len(v['pairs'])==1 and v['managed_used']==0;p=v['pairs'][0];assert p==row['evidence'] and (p['left'],p['right'])==(2,9) and p['support'] is None and p['model_attempted'] is True and p['detail']['terminal']=='inconclusive'
  a,b=map(pixels,row['command'][5:7]);different=sum(x!=y for x,y in zip(a,b))/len(a)
  if row['case']=='flat_brightness':assert set(a)=={64} and set(b)=={192};corr=None
  else:
   assert different>0.98 and len(set(a))==len(set(b))==256
   ma=sum(a)/len(a);mb=sum(b)/len(b);corr=sum((x-ma)*(y-mb) for x,y in zip(a,b))/math.sqrt(sum((x-ma)**2 for x in a)*sum((y-mb)**2 for y in b));assert abs(corr)<0.05
  rss=int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1]);assert rss==row['max_rss_bytes'];rows.append({'case':row['case'],'support':None,'model_attempted':True,'different_pixel_fraction':different,'raw_gray_correlation':corr,'managed_used':0,'elapsed_seconds':row['elapsed_seconds'],'max_rss_bytes':rss})
 out=D/'dedup-native-bank-generated-negatives-audit-20261011.json';assert not out.exists();pins.update({str(p):sha(p) for p in [report,Path(__file__)]});out.write_text(json.dumps({'status':'verified_actual_cpu_two_generated_negative_controls','rows':rows,'pins':pins,'scope':'Actual native/opposite/learned-bank file collection controls on independently generated uniform-brightness and seeded-noise inputs; both typed inconclusive, no false support, managed release. Source/runtime/input/binary evidence pinned; not real burst/semantic/look-alike/general precision or full corpus/RSS qualification.'},indent=2)+'\n');print(rows)
if __name__=='__main__':main()
