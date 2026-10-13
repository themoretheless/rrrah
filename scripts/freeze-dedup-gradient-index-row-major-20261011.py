#!/usr/bin/env python3
import hashlib,json,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
DEST=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-index-row-major-snapshot-20261011')
OUT=D/'dedup-gradient-index-row-major-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not DEST.exists()
 pp=D/'dedup-outcome-native-profile-snapshot-20261011.json';parent=json.loads(pp.read_text());source=Path(parent['snapshot'])
 for p,h in parent['file_hashes'].items():
  assert sha(source/p)==h,p;target=DEST/p;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source/p,target)
 name='crates/rrrah-dedup/src/gradient_index.rs';p=DEST/name;s=p.read_text()
 a=s.index('    pub fn new(');b=s.index('    /// Search a horizontally',a)
 reference=s[a:b].replace('    pub fn new(',"    fn reference<'a>(").replace('Result<Self, LocalError>',"Result<GradientDescriptorIndex<'a>, LocalError>").replace('Ok(Self {','Ok(GradientDescriptorIndex {')
 s=s.replace('        for (_, d) in entries {','        let mut low = [f64::INFINITY; 128];\n        let mut high = [f64::NEG_INFINITY; 128];\n        for (_, d) in entries {',1)
 old='''            if !valid(d) {
                return Err(LocalError::Invalid);
            }
'''
 s=s.replace(old,old+'''            for (dim, value) in d.0.iter().copied().enumerate() {
                low[dim] = low[dim].min(value);
                high[dim] = high[dim].max(value);
            }
''',1)
 old='''            let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
            for (_, d) in entries {
                low = low.min(d.0[dim]);
                high = high.max(d.0[dim]);
            }
            if high - low > widest {
                widest = high - low;
'''
 assert old in s;s=s.replace(old,'''            let span = high[dim] - low[dim];
            if span > widest {
                widest = span;
''',1)
 helper=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-index-row-major-tests-20261011.rs')
 s+='\n#[cfg(test)]\nmod row_major_tests {\nuse super::*;\n'+reference+helper.read_text()+'\n}\n';p.write_text(s)
 hashes={p:sha(DEST/p) for p in parent['file_hashes']}
 OUT.write_text(json.dumps({'snapshot':str(DEST),'parent_manifest':str(pp),'parent_manifest_sha256':sha(pp),'changes':[name],'file_hashes':hashes,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__).resolve()),'test_helper':str(helper),'test_helper_sha256':sha(helper),'scope':'Candidate only; accumulate exact per-coordinate extrema during sequential validation, preserving all index construction callbacks. Root production remains unchanged until qualification and measurements.'},indent=2)+'\n')
 print('frozen candidate',len(hashes))
if __name__=='__main__':main()
