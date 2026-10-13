#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 bp=D/'dedup-loftr-selected-windows-collection-build-20261012.json';b=json.loads(bp.read_text());assert b['status']=='complete_two_preserved_executables_not_executed';assert all(sha(p)==h for p,h in b['pins'].items());assert [x['mode'] for x in b['builds']]==['baseline','selected']
 original=T/'rrrah-disc21-heldout-negative-harness-20261012';pins=dict(b['pins']);rows=[]
 for row in b['builds']:
  mode=row['mode'];mp=D/('dedup-loftr-selected-windows-current-baseline-20261012.json' if mode=='baseline' else 'dedup-loftr-selected-windows-source-v2-20261012.json');m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
  if mode=='baseline':
   for p,h in m['file_hashes'].items():
    if p.startswith(('crates/rrrah-dedup/src/','vendor/loftr/src/')):assert sha(R/p)==h,p
  h=T/f'rrrah-loftr-selected-windows-collection-{mode}-20261012';assert (h/'Cargo.lock').read_bytes()==(original/'Cargo.lock').read_bytes();assert (h/'src/bin'/f'selected_window_collection_{mode}.rs').read_bytes()==(original/'src/bin/disc21_heldout_negative_collection.rs').read_bytes()
  assert row['returncode']==0;raw=Path(row['executable']).read_bytes();assert raw[:4]==b'\xcf\xfa\xed\xfe' and int.from_bytes(raw[4:8],'little')==0x100000c;rows.append({'mode':mode,'executable':row['executable'],'sha256':sha(row['executable']),'source_manifest':str(mp)})
 out=D/'dedup-loftr-selected-windows-collection-build-audit-20261012.json';assert not out.exists();pins.update({str(p):sha(p) for p in [bp,Path(__file__)]});out.write_text(json.dumps({'status':'verified_two_native_arm64_public_collection_builds_unchanged_caller_policy','builds':rows,'pins':pins,'scope':'Baseline dedup and LoFTR source equals current root; selected candidate differs solely in fine-window production method and private test oracle/controls. Both actual caller probes and locked dependencies identical. Compilation only; no end-to-end outcome, recall, total memory cap or adoption claim.'},indent=2)+'\n');print('Verified both public collection executables and unchanged caller policy')
if __name__=='__main__':main()
