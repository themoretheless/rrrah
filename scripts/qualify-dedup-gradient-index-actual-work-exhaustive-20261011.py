#!/usr/bin/env python3
import hashlib,json,re,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';A=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-index-actual-work-exhaustive-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    assert not A.exists();A.mkdir()
    paths=[R/'Cargo.toml',R/'Cargo.lock',*sorted((R/'crates/rrrah-dedup').rglob('*.rs')),R/'crates/rrrah-dedup/Cargo.toml']
    hashes={str(p):sha(p) for p in paths};log=D/'dedup-gradient-index-actual-work-exhaustive-suite-20261011.log';text=log.read_text();rows=[]
    for name,count in [('gradient_index',13),('gradient_index_reflection',2)]:
        match=re.search(r'Running tests/'+name+r'\.rs \(([^)]+)\)',text);assert match
        source=R/match[1];target=A/name;digest=sha(source);shutil.copy2(source,target);assert sha(target)==digest
        result=subprocess.run([str(target)],capture_output=True,text=True)
        output=A/(name+'.log');output.write_text(result.stdout+result.stderr)
        assert result.returncode==0 and re.findall(r'test result: ok\. (\d+) passed; 0 failed;',result.stdout)==[str(count)]
        assert sha(target)==digest
        rows.append({'suite':name,'tests':count,'binary':str(target),'binary_sha256':digest,'log':str(output),'log_sha256':sha(output)})
    assert all(sha(p)==h for p,h in hashes.items())
    out={'status':'complete_bounded_exhaustive_pair_union_oracle','tests':15,'oracle_configurations':176,'oracle_api_invocations':352,'source_hashes':hashes,'results':rows,'producer_sha256':sha(Path(__file__)),'original_log_sha256':sha(log),'scope':'Current macOS raster index suites, copied programs; exhaustive distances and independent reflection permutation on 0/1/2/7/32 descriptors, repeated owners/ambiguous descriptors, inclusive boundaries and exact measured caps. Not corpus or platform qualification.'}
    (D/'dedup-gradient-index-actual-work-exhaustive-audit-20261011.json').write_text(json.dumps(out,indent=2)+'\n');print(out['status'])
if __name__=='__main__':main()
