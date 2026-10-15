from pathlib import Path
import subprocess,json,hashlib,sys,tempfile
R=Path(__file__).resolve().parents[3];D=Path(__file__).parent;W=R/'.dedup-work/estimate157-20261015';run=int(sys.argv[1]);E=D/f'estimate157-native-run{run}-20261015';assert not E.exists();E.mkdir();rows=[];h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def invoke(name,exe,args):
 r=subprocess.run([str(W/exe),*map(str,args)],capture_output=True,text=True,timeout=25);p=E/(name+'.log');p.write_text(r.stdout+r.stderr);rows.append(dict(case=name,returncode=r.returncode,log=str(p.relative_to(R)),sha256=h(p)));return r
with tempfile.TemporaryDirectory(dir=W,prefix='controls-') as tmp:
 p=Path(tmp);a=p/'a';b=p/'b';a.write_bytes(b'ABC');b.write_bytes(b'ABC')
 r=invoke('help','estimator',['--help']);assert r.returncode==1 and 'Usage:' in r.stdout
 r=invoke('missing-file-success','estimator',[p/'missing']);assert r.returncode==0 and 'Error opening file' in r.stderr and 'Total bytes in all files: 0' in r.stdout
 r=invoke('tensor-flag-without-files','estimator',['-t']);assert r.returncode==0 and 'Total bytes in all files: 0' in r.stdout
 r=invoke('single-short-file','estimator',[a]);assert r.returncode==0 and 'Total deduped bytes: 3' in r.stdout
 r=invoke('same-short-files','estimator',[a,b]);assert r.returncode==0 and 'Total bytes in all files: 6' in r.stdout and 'Total deduped bytes: 3' in r.stdout;out=Path(str(b)+'.dedupe_image.ppm');assert out.read_bytes().startswith(b'P6\n256 256\n255\n')
 out.write_bytes(b'OWNED_MARKER');r=invoke('overwrite-existing-sidecar','estimator',[a,b]);assert r.returncode==0 and out.read_bytes().startswith(b'P6\n') and out.read_bytes()!=b'OWNED_MARKER'
 r=invoke('final-compressed-uninitialized-pattern','helper-pattern',['tail',a]);assert r.returncode==0 and 'FINAL_COMPRESSED=aaaaaaaaaaaaaaaa' in r.stdout
 r=invoke('seeded-hash-collision-no-byte-check','helper-pattern',['collision',a]);assert r.returncode==0 and 'SEEDED_HASH_COLLISION_TRUSTED' in r.stdout
 r=invoke('lz4-33-roundtrip-split-oracles','helper-pattern',['lz4']);assert r.returncode==0 and 'LZ4_ROUNDTRIP_AND_SPLIT_ORACLES=33' in r.stdout
 bad=p/'high-byte';bad.write_bytes(b'\xff');r=invoke('high-byte-signed-index-sanitizer','estimator-sanitize',[bad]);assert r.returncode!=0 and 'index -1 out of bounds' in r.stderr and 'uint64_t[256]' in r.stderr
 files=[]
 for i in range(17):
  f=p/f'unique-{i}';f.write_bytes(bytes([33+i]));files.append(f)
 r=invoke('seventeenth-file-color-index-sanitizer','estimator-sanitize',files);assert r.returncode!=0 and 'AddressSanitizer: global-buffer-overflow' in r.stderr and "global variable 'COLORS'" in r.stderr
 empty=p/'empty';empty.write_bytes(b'');r=invoke('empty-file','estimator',[empty]);assert r.returncode==0 and 'Total deduped bytes: 0' in r.stdout
result=dict(status='verified_native_controls_with_expected_diagnostics',controls=len(rows),rows=rows,binaries=[dict(path=str(W/n),sha256=h(W/n)) for n in ['estimator','estimator-sanitize','estimator-pattern','helper-pattern']],compiler=subprocess.check_output(['clang++','--version'],text=True),scope='Unchanged entire estimator and vendored LZ4 compiled onMacARM, actualCLI plus whole-source include helper. Pattern-init compiler diagnostic explicitly distinguishes missing final compressed_len assignment. Seeded map collision is injected, not found naturalhash collision. UBSan/ASan expected failures retained.33LZ4roundtrip/split oracles, tinyownedfiles, no speed/storage/precision/platform claim.')
(E/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(dict(status=result['status'],controls=len(rows),run=run)))
