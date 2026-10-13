#!/usr/bin/env python3
import hashlib,json,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');A=T/'rrrah-gradient-corpus-pyramid-index-qualified-20261011'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 indexpath=D/'dedup-gradient-corpus-pyramid-index-20261011.json';index=json.loads(indexpath.read_text());assert index['status']=='complete_bounded_normalized_corpus_index_oracle';assert all(sha(p)==h for p,h in index['pins'].items())
 for row in index['results']:
  assert row['returncode']==0 and row['status']=='passed_oracle_equality'
  for path,key in [('log','log_sha256'),('paths','paths_sha256'),('descriptors_path','descriptors_sha256')]:assert sha(row[path])==row[key]
 corpus=T/'rrrah-dedup-copydays/prepared.json';c=json.loads(corpus.read_text());files=sorted(c['images']['original'],key=lambda x:x['filename'])+sorted(c['images']['strong'],key=lambda x:x['filename']);dist=[]
 for line in (A/'all386.minimum-distances.tsv').read_text().splitlines():
  a,b,d=line.split('\t');a=int(a);b=int(b);d=float(d);assert d>=0 and not math.isnan(d);dist.append((a,b,d))
 assert [(i,j) for i,j,d in dist]==[(i,j) for i in range(386) for j in range(i+1,386)]
 positives=[(a,b,d) for a,b,d in dist if a<157<=b and files[a]['group_id']==files[b]['group_id']];assert len(positives)==229
 worst=max(d for a,b,d in positives);sweep=[]
 for radius in [0.,0.05,0.1,0.2,0.3,0.5,0.75,1.,worst,4.]:
  selected=[(a,b,d) for a,b,d in dist if d<=radius];sweep.append({'radius':radius,'all_pairs_proposed':len(selected),'publisher_original_query_proposed':sum(d<=radius for a,b,d in positives),'publisher_original_query_total':229})
 actual={tuple(map(int,l.split('\t')[1:])) for l in Path(index['results'][-1]['log']).read_text().splitlines() if l.startswith('P\t')};assert actual=={(a,b) for a,b,d in dist if d<=0.1}
 parentpath=D/'dedup-gradient-corpus-radius-audit-20261011.json';parent=json.loads(parentpath.read_text());parentmin=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-corpus-index-qualified-20261011/all386.minimum-distances.tsv')
 parentdist=[(int(a),int(b),float(v)) for a,b,v in (line.split('\t') for line in parentmin.read_text().splitlines())];assert [(a,b) for a,b,d in parentdist]==[(a,b) for a,b,d in dist]
 # Level zero must survive unchanged when extra pyramid levels are appended.
 # Descriptor bytes provide independent direct evidence, not just recall counts.
 import struct
 def bank(path):
  data=Path(path).read_bytes();assert len(data)%1040==0;out={}
  for off in range(0,len(data),1040):
   owner=struct.unpack_from('<Q',data,off+8)[0];out.setdefault(owner,[]).append(data[off+16:off+1040])
  return out
 old=bank('/Users/themoretheless/.codex/tmp/rrrah-gradient-corpus-index-qualified-20261011/all386.descriptors');new=bank(A/'all386.descriptors');assert set(old)==set(new)
 for owner,descriptors in old.items():assert new[owner][:len(descriptors)]==descriptors
 assert all(d<=old_d for (_,_,d),(_,_,old_d) in zip(dist,parentdist))
 gained=[{'original':files[a]['filename'],'query':files[b]['filename']} for (a,b,d),(_,_,old_d) in zip(dist,parentdist) if a<157<=b and files[a]['group_id']==files[b]['group_id'] and d<=0.1<old_d]
 pins=[Path(__file__),Path('scripts/dedup-gradient-corpus-pyramid-radius-probe-20261011.rs'),T/'rrrah-gradient-corpus-pyramid-radius-probe-20261011',A/'all386.descriptors',A/'all386.minimum-distances.tsv',D/'dedup-gradient-corpus-pyramid-radius-20261011.log',indexpath,parentpath,parentmin,corpus]
 report={'status':'complete_bounded_four_level_corpus_radius_calibration','file_pairs':len(dist),'publisher_original_query_pairs':229,'largest_positive_minimum_distance':worst,'sweep':sweep,'gained_positive_pairs_radius_0_1':gained,'base_descriptor_bytes_preserved_all_files':True,'all_pair_minima_nonincreasing':True,'pins':{str(p.resolve()):sha(p) for p in pins},'limits':'In-sample normalized four-level32-features-per-level interpolated descriptors, maximum128 per file. Scalar minima and index agree. No held-out, semantic-negative, original-resolution or universal-completeness claim; nonproposals require fallback.'}
 (D/'dedup-gradient-corpus-pyramid-radius-audit-20261011.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'sweep':sweep,'gained':len(gained)}))
if __name__=='__main__':main()
