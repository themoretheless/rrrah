"""Fast isolated mathematical feedback; does not qualify the crate or file lifecycle."""
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-gradient-mutual-hoisted-math-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-mutual-hoisted-math-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists()
 manifest_path=D/'dedup-gradient-mutual-hoisted-snapshot-20261011.json';m=json.loads(manifest_path.read_text());snapshot=Path(m['snapshot'])
 source=snapshot/'crates/rrrah-dedup/src/gradient.rs';tests=snapshot/'crates/rrrah-dedup/src/gradient_matching_tests.rs'
 assert sha(source)==m['file_hashes']['crates/rrrah-dedup/src/gradient.rs'] and sha(tests)==m['file_hashes']['crates/rrrah-dedup/src/gradient_matching_tests.rs']
 text=source.read_text();body=text[text.index('fn distances_four('):text.index('\n/// Describe a rotated')]
 ARCHIVE.mkdir();unit=ARCHIVE/'matcher.rs';binary=ARCHIVE/'matcher'
 unit.write_text('''#[derive(Debug, Clone, Copy, PartialEq)] struct GradientDescriptor([f64;128]);
#[derive(Debug, Clone, Copy, PartialEq)] struct GradientFeature {position:[f64;2], descriptor:GradientDescriptor}
#[derive(Debug,Clone,Copy)] struct GradientMatchPolicy {max_comparisons:u64,max_squared_distance:f64,squared_ratio:f64}
#[derive(Debug)] enum LocalError {Invalid,Cancelled,Budget}
mod geometry { #[derive(Debug)] pub struct Correspondence {pub source:[f64;2],pub target:[f64;2]} }
'''+body+'\n#[path = '+json.dumps(str(tests))+'] mod matching_tests;\n')
 build=subprocess.run(['rustc','--edition=2024','--test','-O',str(unit),'-o',str(binary)],capture_output=True,text=True);assert build.returncode==0,build.stderr
 run=subprocess.run([str(binary)],capture_output=True,text=True);assert run.returncode==0 and '4 passed; 0 failed; 1 ignored' in run.stdout,(run.stdout,run.stderr)
 log=ARCHIVE/'tests.log';log.write_text(run.stdout+run.stderr)
 report={'status':'complete_isolated_four_math_oracle_tests','tests':4,'ignored':1,'pins':{str(p):sha(p) for p in [manifest_path,source,tests,unit,binary,log,Path(__file__)]},'scope':__doc__,'limits':['Extracted production distance/matching functions and frozen oracle test module; simple type shells only','Does not test crate integration, decode, source guards, managed credit, Torch or native collection behavior','Performance retention requires separate real-photo comparisons']}
 OUT.write_text(json.dumps(report,indent=2)+'\n');print(run.stdout)
if __name__=='__main__':main()
