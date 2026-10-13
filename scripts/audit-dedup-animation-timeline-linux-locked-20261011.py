#!/usr/bin/env python3
"""Archive the completed frozen projected-portfolio Linux gate before target reuse."""
import gzip,hashlib,json,re,shutil,subprocess
from collections import Counter
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-animation-timeline-linux-locked-snapshot-20261011.json'
LOG=D/'dedup-animation-timeline-linux-locked-20261011.log'
OUT=D/'dedup-animation-timeline-linux-locked-audit-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-animation-timeline-linux-locked-qualified-20261011')
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native')
HELPER=Path('/Users/themoretheless/.codex/tmp/rrrah-animation-timeline-linux-locked-20261011.sh')
EXPECTED={'lib':6,'animated':7,'sequence':7}
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists()
 text=LOG.read_text();assert 'test result: FAILED' not in text
 runs=re.findall(r'Running (?:unittests src/lib\.rs|tests/([^\s]+)\.rs) \(([^)]+)\)(.*?)(?=\n\s*Running |\Z)',text,re.S)
 # Cargo's stderr headings can precede the previous binary's stdout summary.
 # Bind exact test names to each unchanged ELF instead of assigning by adjacency.
 assert len(runs)==len(EXPECTED)
 binaries=[binary for _,binary,_ in runs]
 command=['docker','run','--rm','--user','65534:65534',
  '-v',str(BASE)+':/validation:ro',
  '-v','/opt/rrrah-dedup-validation/sysroot:/validation/sysroot:ro',
  '-e','LD_LIBRARY_PATH=/validation/sysroot/usr/lib/aarch64-linux-gnu:/validation/sysroot/usr/lib/llvm-18/lib',
  'ubuntu:24.04','sh','-c',
  'for binary in "$@"; do printf "RRRAH_LIST %s\\n" "$binary"; case "$binary" in *rrrah_dedup-*) "$binary" animated:: --list ;; *) "$binary" --list ;; esac || exit; printf "RRRAH_IGNORED %s\\n" "$binary"; case "$binary" in *rrrah_dedup-*) "$binary" animated:: --list --ignored ;; *) "$binary" --list --ignored ;; esac || exit; done',
  'sh',*binaries]
 listed=subprocess.run(command,capture_output=True,text=True);assert listed.returncode==0,listed.stderr
 listing=ROOT/'docs/research/dedup-animation-timeline-linux-locked-platform-test-list-20261011.log'
 listing.write_text(listed.stdout+listed.stderr)
 all_names={};ignored_names={};current=None;ignored=False
 for line in listed.stdout.splitlines():
  if line.startswith('RRRAH_LIST '):current=line[11:];ignored=False;all_names[current]=[]
  elif line.startswith('RRRAH_IGNORED '):current=line[14:];ignored=True;ignored_names[current]=[]
  elif line.endswith(': test'):
   (ignored_names if ignored else all_names)[current].append(line[:-6])
 observed={};expected_passed=Counter();expected_ignored=Counter()
 for name,binary,_ in runs:
  name=name or 'lib';assert name not in observed
  ignored_set=set(ignored_names[binary]);names=all_names[binary]
  assert ignored_set.issubset(names)
  passed=[n for n in names if n not in ignored_set]
  observed[name]=len(passed);expected_passed.update(passed);expected_ignored.update(ignored_set)
 assert observed==EXPECTED,observed
 passed=Counter(re.findall(r'^test (\S+) \.\.\. ok$',text,re.M))
 ignored=Counter(re.findall(r'^test (\S+) \.\.\. ignored(?:,.*)?$',text,re.M))
 assert passed==expected_passed,(passed-expected_passed,expected_passed-passed)
 assert ignored==expected_ignored,(ignored-expected_ignored,expected_ignored-ignored)
 summaries=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)
 assert len(summaries)==len(EXPECTED) and sum(int(n) for n,_ in summaries)==sum(EXPECTED.values())
 assert sum(int(n) for _,n in summaries)==sum(expected_ignored.values())
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot'])
 def verify():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify();ARCHIVE.mkdir();artifacts=[]
 for name,binary,_ in runs:
  source=BASE/Path(binary).relative_to('/validation');digest=sha(source)
  with source.open('rb') as f:assert f.read(4)==b'\x7fELF'
  target=ARCHIVE/(source.name+'.gz')
  with source.open('rb') as f,target.open('wb') as out:
   with gzip.GzipFile(filename='',mode='wb',fileobj=out,compresslevel=1,mtime=0) as z:shutil.copyfileobj(f,z,1024*1024)
  h=hashlib.sha256()
  with gzip.open(target,'rb') as f:
   for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
  assert h.hexdigest()==digest==sha(source)
  artifacts.append({'suite':name or 'lib','source':str(source),'archive':str(target),
   'executable_sha256':digest,'archive_sha256':sha(target),'executable_bytes':source.stat().st_size})
 pins={}
 for p in [MANIFEST,LOG,HELPER,listing,Path(__file__)]:
  pins[str(p)]=sha(p);shutil.copy2(p,ARCHIVE/p.name);assert sha(ARCHIVE/p.name)==pins[str(p)]
 verify()
 report={'status':'complete_bounded_frozen_animation_timeline_linux_gate','tests':sum(observed.values()),
  'programs':len(artifacts),'suite_counts':observed,'log_binding':'Exact --list/--ignored names from each unchanged ELF matched as multisets against full completed output; summaries checked globally because headings interleave','ignored_tests':sum(int(n) for n in re.findall(r'test result: ok\. \d+ passed; 0 failed; (\d+) ignored;',text)),'frozen_files':len(m['file_hashes']),
  'artifacts':artifacts,'pins':pins,
  'scope':'Linux aarch64 frozen animation unit, real GIF/APNG/WebP integration and sequence oracle tests; artifacts archived before target reuse',
  'limits':['Bounded fixtures do not establish broad collection recall or precision',
            'No Torch, Windows, broad negative/burst or hard total RSS qualification',
            'Before/after source reads do not establish an atomic working-tree capture; frozen file bytes are authoritative',
            'The earlier snapshot scope used atomically imprecisely; no atomic filesystem snapshot was created',
            'Full229 uses an earlier independent frozen source']}
 OUT.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['tests'])
if __name__=='__main__':main()
