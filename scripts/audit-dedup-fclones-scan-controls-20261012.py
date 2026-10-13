#!/usr/bin/env python3
import hashlib,json,re,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'fclones-hasher-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot'])
 assert subprocess.check_output(['git','-C',str(s),'rev-parse','HEAD'],text=True).strip()==m['commit']
 assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 c=R/m['control'];assert sha(c)==m['control_sha256'];src=(s/'fclones/src/hasher.rs').read_text();a=src.index('thread_local! {');b=src.index('/// Computes the hash value',a);assert src[a:b] in c.read_text()
 license=D/'fclones-controls-LICENSE-20261012';assert license.read_bytes()==(s/'LICENSE').read_bytes()
 binary=T/'rrrah-fclones-scan-controls-20261012';log=T/'rrrah-fclones-scan-controls-20261012.log';listed=subprocess.check_output([str(binary),'--list'],text=True);expected=set(re.findall(r'^(.*): test$',listed,re.M));passed=set(re.findall(r'^test (\S+) \.\.\. ok$',log.read_text(),re.M));assert len(expected)==6 and expected==passed;assert '6 passed; 0 failed; 0 ignored' in log.read_text()
 out=D/'fclones-scan-controls-audit-20261012.json';assert not out.exists()
 out.write_text(json.dumps({'status':'verified_six_original_scan_helper_controls','pins':{str(p):sha(p) for p in [mp,c,license,binary,log,Path(__file__)]},'tests':sorted(passed),'findings':['Requested prefix and short reads are handled by the original helper.','Early EOF returns short successful length; caller must validate promised length.','A zero buffer length on an empty thread-local buffer returns zero bytes despite nonempty input.','Interrupted errors propagate without retry.','Largest per-thread buffer remains retained after smaller requests.'],'scope':'Original scan and TLS block executed verbatim with primitive FileLen adapter; default FileHasher buffer is nonzero 65536. Zero-buffer control is helper contract boundary, not normal CLI failure. Early EOF is documented at-most stream behavior; no whole scanner mutation race or bad duplicate group claim. This is targeted hasher review, not full42-file source review, whole upstream suite or process memory qualification. No production adoption.'},indent=2)+'\n');print('Verified six original fclones scan helper controls; full tree review remains open')
if __name__=='__main__':main()
