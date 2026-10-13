#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-loftr-block-masks-native-snapshot-20261011.json';m=json.loads(parent.read_text());S=Path(m['snapshot']);A=T/'rrrah-model-points-trace-snapshot-20261011';out=D/'dedup-model-points-trace-snapshot-20261011.json';assert not A.exists() and not out.exists();A.mkdir();hashes=m['file_hashes'].copy()
 for f,h in hashes.items():
  assert sha(S/f)==h;q=A/f;q.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(S/f,q)
 f='crates/rrrah-dedup/src/model_compare.rs';p=A/f;s=p.read_text();needle='        let rd = right_input.original_dimensions();\n';assert s.count(needle)==1;s=s.replace(needle,needle+'''        // Diagnostic-only exact-bit trace after selection/full-domain mapping.
        eprintln!("RRRAH_MODEL_POINTS\\t{}\\t{:?}\\t{:?}\\t{:?}\\t{:?}\\t{:?}",
            raw_proposals, ld, rd, left_input.rectangle(), right_input.rectangle(),
            selected.iter().map(|p| [p.source[0].to_bits(),p.source[1].to_bits(),p.target[0].to_bits(),p.target[1].to_bits()]).collect::<Vec<_>>());
''');p.write_text(s);hashes[f]=sha(p);probe='scripts/dedup-loftr-native-probe/src/bin/model_points_trace_files.rs';p=A/probe;shutil.copy2(R/probe,p);hashes[probe]=sha(p);assert all(sha(A/f)==h for f,h in hashes.items());out.write_text(json.dumps({'snapshot':str(A),'parent_manifest':str(parent),'parent_sha256':sha(parent),'file_hashes':hashes,'changes':[f,probe],'producer':str(Path(__file__)),'producer_sha256':sha(Path(__file__)),'scope':'Diagnostic-only model selected-point/full-domain mapping trace. Actual fitting and pixel algorithm unchanged; tracing scratch/logs unmanaged. Parent blocked-mask LoFTR and compatible decoder/lock retained.'},indent=2)+'\n');print(out)
if __name__=='__main__':main()
