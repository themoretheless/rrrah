#!/usr/bin/env python3
"""Verify pinned original-frame replay, candidate provenance and refusal logic."""
from pathlib import Path
import hashlib,json,re,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-exposure-replay-audit-20261011.json';assert not out.exists()
 mp=D/'dedup-exposure-calibration-snapshot-20261011.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256']
 ip=D/'dedup-exposure-replay-input-20261011.json';inp=json.loads(ip.read_text());op=D/'dedup-exposure-replay-output-20261011.json';obs=json.loads(op.read_text());assert obs['managed_used']==0
 bp=D/'dedup-disc21-overlap-regions-native-v2-20261011.json';base=json.loads(bp.read_text());be=json.loads(base['stdout'])['evidence'];assert [inp['left'],inp['right']]==base['command'][5:7]
 for bank in [6,7]:
  original=next(a for a in be['pixel_attempts'] if a['bank']==bank and a['window_radius']==2)
  current=next(c for c in inp['cases'] if c['name']=='overlap_bank'+str(bank))
  assert current['transform']==original['transform'] and current['witnesses']==original['witnesses']
  raw=next(r for r in obs['evidence']['rows'] if r['case']==current['name'] and not r['calibrated'] and r['rank']==8)
  assert raw['result']=='Ok('+original['pixels']+')'
 gp=D/'dedup-disc21-dark-contrast-grid-20261011.json';grid=json.loads(gp.read_text());line=next(x for x in grid['stderr'].splitlines() if x.startswith('TRACE candidate bank=12 '));matrix=json.loads(line.split('transform=')[1]);rp=D/'dedup-disc21-dark-contrast-spatial-refusal-20261011.json';ref=json.loads(rp.read_text());c=inp['cases'][2];assert c['name']=='rejected_bank12' and c['transform']==matrix and c['witnesses']==[{'source':p[:2],'target':p[2:]} for p in ref['witnesses']]
 for p in [inp['left'],inp['right']]:assert sha(p)==ref['pins'][p]
 rows=obs['evidence']['rows'];assert len(rows)==12 and len({(r['case'],r['rank'],r['calibrated']) for r in rows})==12
 summary=[]
 for row in rows:
  counts=[tuple(map(int,v)) for v in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',row['result'])];assert len(counts)==2
  supported=all(n>=1000 and a/n>=0.9 for n,a in counts);assert row['supported']==supported and ('supported: true' in row['result'])==supported
  if not row['calibrated']:assert counts==[(0,0),(0,0)]
  summary.append({k:row[k] for k in ['case','rank','calibrated','supported']}|{'counts_informative_agreeing':counts})
 accepted=[(r['case'],r['rank']) for r in rows if r['supported']];assert accepted==[('overlap_bank7',2),('rejected_bank12',2)]
 h=T/'rrrah-exposure-replay-harness-20261011';source=h/'src/main.rs';assert sha(source)==sha(R/'scripts/dedup-exposure-replay/src/main.rs');text=source.read_text()
 for literal in ['minimum_contrast:0.005','minimum_pairs:1000','minimum_agreement:0.9','minimum_witnesses:10','filter_radius:12','ls.verify','rs.verify']:assert literal in text
 build=D/'dedup-exposure-replay-build-v2-20261011.log';assert 'Finished `dev` profile' in build.read_text()
 binary=T/'rrrah-opposite-native-snapshot-20261011/target/debug/dedup-exposure-replay';saved=T/'rrrah-exposure-replay-qualified-20261011';assert not saved.exists();saved.mkdir();shutil.copy2(binary,saved/binary.name)
 paths=[mp,ip,op,bp,gp,rp,source,h/'Cargo.toml',h/'Cargo.lock',build,D/'dedup-exposure-replay-runtime-20261011.log',saved/binary.name,Path(__file__),Path(inp['left']),Path(inp['right'])]
 report={'status':'verified_original_pixel_calibration_replay_rejected_for_general_adoption','pins':{str(p):sha(p) for p in paths},'evidence':obs,'rows':summary,'adoption':'rejected_for_general_search','scope':'Original JPEGs and saved candidates only; no model extraction rerun. Real overlap bank7 recovered at rank2, but misplaced bank12 also supported at rank2. Exposure alone does not qualify copy localization or general precision. Per-call rank work admitted, both calibrated buffers use managed credits for full lifetime; diagnostic JSON/Vec metadata external. Public byte snapshots guard original JPEGs; no external ICC/profile-sidecar case tested. Shared-host timing is not a performance qualification.'}
 out.write_text(json.dumps(report,indent=2)+'\n');print(report['status']);print('accepted',accepted)
if __name__=='__main__':main()
