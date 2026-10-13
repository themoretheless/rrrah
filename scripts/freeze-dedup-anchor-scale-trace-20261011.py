#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'dedup-anchor-window-trace-snapshot-20261011.json';m=json.loads(mp.read_text());old=Path(m['harness']);assert all(sha(old/f)==h for f,h in m['harness_hashes'].items());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items())
 H=T/'rrrah-anchor-scale-trace-harness-20261011';out=D/'dedup-anchor-scale-trace-harness-20261011.json';assert not H.exists() and not out.exists();shutil.copytree(old,H);p=next((H/'src/bin').glob('*.rs'));s=p.read_text();assert 'args.len()!=3' in s;s=s.replace('args.len()!=3','args.len()!=4').replace('input.json trials seed','input.json trials seed filter-radius');s=s.replace('let policy=native_policy::portfolio(tol);','let mut policy=native_policy::portfolio(tol);let filter_radius:u32=args[3].parse()?;if filter_radius>32{return Err("filter limit".into());}policy.search.anchors.local.filter_radius=filter_radius;');s=s.replace('"points":points.len(),"decimal_bit_differences"','"filter_radius":filter_radius,"points":points.len(),"decimal_bit_differences"');p.write_text(s)
 out.write_text(json.dumps({'harness':str(H),'parent_manifest':str(mp),'parent_sha256':sha(mp),'files':{str(p.relative_to(H)):sha(p) for p in H.rglob('*') if p.is_file()},'scope':'Explicit diagnostic filter radius0..32; same admitted pixel/site limits, full point set, geometry, contrast, agreement and minimum witnesses. No production defaults or source change.'},indent=2)+'\n')
if __name__=='__main__':main()
