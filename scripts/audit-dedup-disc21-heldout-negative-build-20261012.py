#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 bp=D/'dedup-disc21-heldout-negative-build-20261012.json';b=json.loads(bp.read_text());assert b['build_returncode']==0
 assert all(sha(p)==h for p,h in b['pins'].items())
 mp=D/'dedup-disc21-heldout-negative-source-20261012.json';m=json.loads(mp.read_text());parent=json.loads(Path(m['parent_manifest']).read_text())
 assert m['deltas_from_parent']==['crates/rrrah-dedup/tests/exact_files.rs']
 for p,h in m['file_hashes'].items():
  assert sha(Path(m['snapshot'])/p)==h
  if p.startswith('crates/rrrah-dedup/src/'):assert sha(R/p)==h
 old=T/'rrrah-disc21-bidirectional-exposure-collection-harness-20261012';new=T/'rrrah-disc21-heldout-negative-harness-20261012'
 a=(old/'src/bin/disc21_bidirectional_exposure_collection.rs').read_text();c=(new/'src/bin/disc21_heldout_negative_collection.rs').read_text()
 a=a.replace('    let mut negative=rrrah_decode::DecodeRequest::new(&args[2]);negative.assume_untagged_linear_srgb=true;\n','').replace('args.len() != 6','args.len() != 5').replace('usage: disc21_bidirectional_exposure_collection source target negative weights max_side source_tolerance','usage: disc21_heldout_negative_collection source target weights max_side source_tolerance')
 for i in (3,4,5):a=a.replace(f'args[{i}]',f'args[{i-1}]')
 a=a.replace('Actual public bidirectional collection with reference assigned lower ID than dark query. One cached CPU provider spans both directions. Development control, not general precision/RSS/platform/full500 qualification.','Actual public bidirectional collection on precommitted unused publisher distractor query/reference pair. Reference has lower ID; one CPU provider spans both directions. Fixed previous caller policy, not full precision/RSS/platform/full500 qualification.')
 assert a==c
 assert (old/'Cargo.lock').read_bytes()==(new/'Cargo.lock').read_bytes()
 exe=Path(b['executable']);raw=exe.read_bytes();assert raw[:4]==b'\xcf\xfa\xed\xfe' and int.from_bytes(raw[4:8],'little')==0x100000c
 out=D/'dedup-disc21-heldout-negative-build-audit-20261012.json';assert not out.exists()
 result={'status':'verified_current_dedup_source_and_unchanged_archived_caller_policy','pins':dict(b['pins'],**{str(p):sha(p) for p in [bp,mp,Path(__file__)]}),'policy_changes':[],'source_delta':'test-only exact block order regression','compiler_current_observation':subprocess.check_output(['rustc','-vV'],cwd='/tmp',text=True),'scope':'Compile and source audit, no execution outcome. Compiler queried after build, not a captured build invocation version. Fixed archived caller assumption for untagged linear sRGB; no default decoder color or precision qualification.'}
 out.write_text(json.dumps(result,indent=2)+'\n');print(result['status'])
if __name__=='__main__':main()
