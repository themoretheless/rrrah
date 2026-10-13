"""Qualify pure raster index availability and archive Windows compile-only artifacts."""
import hashlib,json,re,shutil,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');OUT=D/'dedup-gradient-index-raster-audit-20261011.json';assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
mp=D/'dedup-gradient-index-raster-snapshot-20261011.json';m=json.loads(mp.read_text());parent_path=D/'dedup-gradient-batch-production-snapshot-20261011.json';parent=json.loads(parent_path.read_text())
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
changes=sorted(p for p,h in parent['file_hashes'].items() if p.startswith('crates/rrrah-dedup/') and m['file_hashes'][p]!=h)
assert changes==['crates/rrrah-dedup/src/lib.rs','crates/rrrah-dedup/tests/gradient_index.rs']
oldlib=(Path(parent['snapshot'])/changes[0]).read_text();newlib=(Path(m['snapshot'])/changes[0]).read_text();assert newlib==oldlib.replace('#[cfg(feature = "decode")]\npub mod gradient_index;','#[cfg(feature = "raster")]\npub mod gradient_index;',1)
oldtest=(Path(parent['snapshot'])/changes[1]).read_text();newtest=(Path(m['snapshot'])/changes[1]).read_text();assert newtest==oldtest.replace('#![cfg(feature = "decode")]','#![cfg(feature = "raster")]',1)
assert 'decode = ["raster", "dep:rrrah-decode"]' in (Path(m['snapshot'])/'crates/rrrah-dedup/Cargo.toml').read_text()
mac=D/'dedup-gradient-index-raster-macos-20261011.log';counts=[int(x) for x in re.findall(r'test result: ok\. (\d+) passed; 0 failed;',mac.read_text())];assert counts==[22,7,2],counts
win=D/'dedup-gradient-index-raster-windows-build-20261011.log';default=D/'dedup-gradient-batch-windows-core-build-20261011.log'
compiled=[]
for source,expected,variant in [(default,1,'parent-default-core'),(win,125,'fixed-raster')]:
 log=source.read_text();assert 'Finished `test` profile' in log and 'error: could not compile' not in log
 rows=re.findall(r'Executable ([^\n(]+) \(([^\n)]+\.exe)\)',log);assert len(rows)==expected,(source,len(rows))
 for label,path in rows:
  p=Path(path);p=p if p.is_absolute() else ROOT/p;assert p.parent==ROOT/'target/x86_64-pc-windows-gnu/debug/deps';assert p.is_file() and not p.is_symlink();compiled.append((variant,label.strip(),p))
archive=T/'windows-gradient-index-raster-qualified-20261011';assert not archive.exists();archive.mkdir()
artifacts=[]
for variant,label,p in compiled:
 dest=archive/p.name;assert not dest.exists();before=sha(p);shutil.copy2(p,dest);assert sha(dest)==before and sha(p)==before
 with dest.open('rb') as f:
  header=f.read(64);assert header[:2]==b'MZ';f.seek(struct.unpack('<I',header[60:64])[0]);assert f.read(6)==b'PE\0\0\x64\x86'
 artifacts.append(dict(variant=variant,label=label,path=str(dest),sha256=before,bytes=dest.stat().st_size))
def run(*args):return subprocess.run(args,capture_output=True,text=True,check=True).stdout.strip()
rustc=Path(run('rustup','which','rustc'));gcc=Path('/opt/homebrew/bin/x86_64-w64-mingw32-gcc');ld=Path(run(str(gcc),'-print-prog-name=ld'));std=Path(run(str(rustc),'--print','target-libdir','--target','x86_64-pc-windows-gnu'))
stdpins={str(p):sha(p) for pattern in ['libstd-*.rlib','libcore-*.rlib','libpanic_unwind-*.rlib'] for p in std.glob(pattern)};assert len(stdpins)==3
files=[mp,parent_path,mac,win,default,D/'dedup-gradient-batch-windows-raster-build-20261011.log',rustc,gcc,ld,Path(__file__)]
a={'status':'complete_raster_feature_gate_macos_runtime_and_windows_compile_only','source_changes':changes,'decode_implies_raster':True,'decode_enabled_numerical_source_unchanged':True,'macos_raster_tests':31,'macos_suite_counts':counts,'windows_fixed_raster_compiled_test_binaries':125,'windows_parent_default_core_compiled_test_binaries':1,'windows_tests_executed':False,'windows_native_runtime_qualified':False,'windows_target':'x86_64-pc-windows-gnu','rustc':run(str(rustc),'-Vv'),'gcc':run(str(gcc),'--version'),'target_standard_library_pins':stdpins,'artifacts':artifacts,'pins':{str(p):sha(p) for p in files},'scope':'Pure managed descriptor index requires raster/rrrah-core, not decoder support. Existing index/reflection tests run without decoder on macOS. All125 raster test programs link for Windows, including source-gated empty programs; this is compile-only, not125 executed test cases. Decoder-enabled bodies unchanged by two feature conditions. Windows file lifecycle/runtime, Windows decoder/Torch, full corpus and all-case completion remain open.'}
OUT.write_text(json.dumps(a,indent=2)+'\n');print(a['status'],len(artifacts),'archived PE64 files')
