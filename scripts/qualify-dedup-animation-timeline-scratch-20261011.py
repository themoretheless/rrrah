#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';A=Path('/Users/themoretheless/.codex/tmp/rrrah-animation-timeline-scratch-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not A.exists();A.mkdir();sources=[R/'Cargo.toml',R/'Cargo.lock',Path(__file__)]
 for crate in ['rrrah-dedup','rrrah-core','rrrah-decode','rrrah-memory']:
  base=R/'crates'/crate;sources += [p for p in base.rglob('*') if p.is_file() and 'target' not in p.parts and p.suffix in ['.rs','.toml']]
 source_hashes={str(p):sha(p) for p in sources};archived={}
 for p in sources:
  target=A/'source'/p.relative_to(R);target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,target);assert sha(target)==source_hashes[str(p)];archived[str(target)]=sha(target)
 rows=[]
 for name,count,filter_name in [('unit',6,'animated::'),('animated',7,None),('sequence',7,None)]:
  log=D/f'dedup-animation-timeline-scratch-{name if name=="unit" else "integration"}-20261011.log';text=log.read_text();pattern = (r'Running unittests src/lib.rs \(([^)]+)\)' if name=='unit' else r'Running tests/'+name+r'.rs \(([^)]+)\)');match=re.search(pattern,text);assert match
  original=R/match[1];binary=A/name;digest=sha(original);shutil.copy2(original,binary);assert sha(binary)==digest
  result=subprocess.run([str(binary),*([filter_name] if filter_name else [])],capture_output=True,text=True);out=A/(name+'.log');out.write_text(result.stdout+result.stderr)
  assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==[str(count)]
  assert sha(binary)==digest;shutil.copy2(log,A/log.name)
  rows.append({'suite':name,'tests':count,'binary':str(binary),'binary_sha256':digest,'log':str(out),'log_sha256':sha(out),'original_log':str(A/log.name),'original_log_sha256':sha(log)})
 assert all(sha(p)==h for p,h in source_hashes.items());assert all(sha(p)==h for p,h in archived.items())
 parent=Path('/Users/themoretheless/.codex/tmp/rrrah-animation-timeline-scratch-parent-20261011.rs');shutil.copy2(parent,A/'animated-parent.rs')
 sequence_parent=Path('/Users/themoretheless/.codex/tmp/rrrah-animation-timeline-sequence-parent-20261011.rs');shutil.copy2(sequence_parent,A/'sequence-parent.rs')
 report={'sequence_parent_sha256':sha(sequence_parent),'status':'complete_bounded_animation_timeline_scratch_ownership','tests':20,'results':rows,'archived_sources':archived,'source_manifest':'Archived current Rust/TOML source files from four local crates plus root manifests; not a newly frozen complete-workspace build','parent_source_sha256':sha(parent),'candidate_source_sha256':sha(R/'crates/rrrah-dedup/src/animated.rs'),'scope':'macOS decode animation unit and integration controls; managed timeline frame-view tables, two-budget and late budget refusal, every cancellation checkpoint, timing/disposal/truncation, independent discrete timeline oracle and managed-credit release. Not hard RSS, all formats, or Linux validation of this animation change.'}
 (D/'dedup-animation-timeline-scratch-audit-20261011.json').write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['tests'])
if __name__=='__main__':main()
