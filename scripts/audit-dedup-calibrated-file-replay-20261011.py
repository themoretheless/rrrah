#!/usr/bin/env python3
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-calibrated-file-replay-audit-20261011.json';assert not out.exists();parent=D/'dedup-disc21-overlap-calibrated-strong-audit-20261011.json';a=json.loads(parent.read_text());assert all(sha(p)==h for p,h in a['pins'].items());mp=D/'dedup-exposure-anchor-files-snapshot-20261011.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items())
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-calibrated-file-replay-output-20261011.json';v=json.loads(op.read_text());assert v['managed_used']==0 and len(v['rows'])==3
 for row,case in zip(v['rows'],inp['cases']):
  assert row['case']==case['name'] and row['witnesses']==len(case['witnesses'])
  if row['witnesses']<20:assert row['result']=='Err(Rank(InsufficientGeometry))' and not row['supported']
  else:
   assert row['supported'];counts=[tuple(map(int,x))for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',row['result'])];assert counts==[(1244,1226),(1545,1494)]
 h=T/'rrrah-calibrated-file-replay-harness-20261011';source=h/'src/main.rs';assert sha(source)==sha(R/'scripts/dedup-calibrated-file-replay/src/main.rs');text=source.read_text();assert 'minimum_witnesses:20' in text and 'compare_exposure_anchor_rank_files' in text
 log=D/'dedup-calibrated-file-replay-build-20261011.log';assert 'Finished `dev` profile' in log.read_text();binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-calibrated-file-replay';saved=T/'rrrah-calibrated-file-replay-qualified-20261011';assert not saved.exists();saved.mkdir();shutil.copy2(binary,saved/binary.name)
 paths=[parent,mp,ip,op,source,h/'Cargo.toml',h/'Cargo.lock',log,D/'dedup-calibrated-file-replay-runtime-20261011.log',saved/binary.name,Path(__file__)];report={'status':'verified_strong_calibrated_file_api_rejects_known_misplaced_candidate','pins':{str(p):sha(p)for p in paths},'evidence':v,'scope':'Actual guarded file API on saved original JPEG candidates. Min20 refuses known misplaced11-witness bank12 and smaller12-witness bank6 explicitly;24-witness bank7 retains automatic positive counts. Development-selected minimum, not held-out precision evidence or proof all false matches have fewer20 witnesses. No default adoption/all-case claim.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'])
if __name__=='__main__':main()
