#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-anchor-window-trace-snapshot-20261011.json';S=T/'rrrah-anchor-window-trace-snapshot-20261011';H=T/'rrrah-anchor-window-trace-harness-20261011';assert not out.exists() and not S.exists() and not H.exists()
 parent=D/'dedup-joint-recovery-native-snapshot-20261011.json';m=json.loads(parent.read_text());P=Path(m['snapshot'])
 for f,h in m['file_hashes'].items():
  assert sha(P/f)==h;p=S/f;p.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(P/f,p)
 rel='crates/rrrah-dedup/src/anchor_rank.rs';p=S/rel;s=p.read_text();needle='            total.sites += e.sites;';assert s.count(needle)==1;s=s.replace(needle,'            eprintln!("RRRAH_ANCHOR_WINDOW\\t{:?}\\t{:?}\\t{:?}\\t{:?}", a.dimensions(), b.dimensions(), rect, e);\n'+needle);p.write_text(s)
 old=T/'rrrah-joint-geometry-sampling-patched-harness-20261011';shutil.copytree(old,H,ignore=shutil.ignore_patterns('target','.git'));p=H/'Cargo.toml';p.write_text(p.read_text().replace(str(P),str(S)))
 files={f:sha(S/f) for f in m['file_hashes']};hfiles={str(p.relative_to(H)):sha(p) for p in H.rglob('*') if p.is_file()}
 out.write_text(json.dumps({'snapshot':str(S),'harness':str(H),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':files,'harness_hashes':hfiles,'deltas':[rel],'scope':'Diagnostic-only per-anchor window count logging after original pixel evaluation; no selection, geometry, pixel threshold, default or public API change. stderr diagnostic storage outside managed payload and no RSS comparison.'},indent=2)+'\n');print(H)
if __name__=='__main__':main()
