#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 mp=D/'find-duplicates-full-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['source']);h=Path(m['control_source']);assert m['commit']=='ea0d9afe789786172b46815162b3464909bc2d2c'
 files=subprocess.check_output(['git','ls-files'],cwd=s,text=True).splitlines();assert len(files)==10 and set(files)==set(m['file_hashes'])==set(m['full_text_read'])
 pins={str(mp):sha(mp),str(Path(__file__)):sha(Path(__file__))}
 for f in files:
  p=s/f;assert p.read_bytes()==subprocess.check_output(['git','show',m['commit']+':'+f],cwd=s);assert sha(p)==m['file_hashes'][f]==sha(h/f);pins[str(p)]=sha(p);pins[str(h/f)]=sha(h/f)
 license=(s/'LICENSE').read_text();assert 'The MIT License (MIT)' in license and 'Copyright (c) 2023 Tom Payne' in license
 controls=D/'find-duplicates-controls-20261012.go';assert sha(controls)==sha(h/'internal/dupfind/rrrah_controls_test.go');pins[str(controls)]=sha(controls);pins[str(h/'internal/dupfind/rrrah_controls_test.go')]=sha(controls)
 upstream=T/'rrrah-find-duplicates-upstream-tests-20261012.jsonl';control=T/'rrrah-find-duplicates-controls-tests-attempt2-20261012.jsonl'
 parsed=[]
 for p in [upstream,control]:
  rows=[json.loads(x) for x in p.read_text().splitlines()];assert not any(x['Action']=='fail' for x in rows);assert any(x['Action']=='pass' and x.get('Package')=='github.com/twpayne/find-duplicates/internal/dupfind' and 'Test' not in x for x in rows);parsed.append(rows);pins[str(p)]=sha(p)
 leaves=[x['Test'] for x in parsed[0] if x['Action']=='pass' and x.get('Test','').startswith('TestDupFinder/')];assert len(leaves)==11
 expected={'TestControlConstantHashAdmitsDifferentBytes','TestControlPreviouslyEmptyFilesAreNotReopened','TestControlHardlinksAppearAsDuplicatePaths'};assert {x.get('Test') for x in parsed[1] if x['Action']=='pass' and 'Test' in x}==expected
 for p in [T/'rrrah-find-duplicates-upstream-tests-20261012.stderr',T/'rrrah-find-duplicates-controls-tests-attempt2-20261012.stderr']:
  assert all(line.startswith('go: downloading ') for line in p.read_text().splitlines());pins[str(p)]=sha(p)
 archive=T/'rrrah-go-1.27.2-20261012/go1.27.2.darwin-arm64.tar.gz';assert sha(archive)=='76812b213b1b2302c978d28fa52fa92d541704b9e7d9d5db8002c50e4018c4c5';pins[str(archive)]=sha(archive)
 go=T/'rrrah-go-1.27.2-20261012/go/bin/go';version=subprocess.check_output([str(go),'version'],text=True).strip();assert 'go1.27.2 darwin/arm64' in version;pins[str(go)]=sha(go)
 r={'status':'verified_all_10_tracked_blobs_11_upstream_cases_3_controls','registry_index':73,'repository':m['repository'],'commit':m['commit'],'runtime':version,'original_test_cases':leaves,'control_tests':sorted(expected),'findings':[
 'Pluggable hash collision groups unequal same-size bytes; SHA256 normal control separates them. This is documented hash-group semantics, insufficient for byte identity.',
 'Original hashPath zero-size shortcut returns empty digest without reopening files that grew after metadata observation. Direct internal helper control; not a full scheduled filesystem race reproduction.',
 'Two paths sharing the same physical hardlink are one hash group; no separate redundant-copy versus alias classification.',
 'Source-only: file hashing starts one goroutine per eligible path without an explicit worker limit; no payload/RSS admission or mutation validation. Source FIXME documents channel lifecycle risk; no crash reproduction claimed.'
 ],'pins':pins,'scope':'Complete pinned text-source review and unchanged upstream tests on macOS ARM Go1.27.2. Independent controls in separate copy, no upstream production code adopted. Not CI lint/race/platform/resource/full500 completion.'}
 out=D/'find-duplicates-full-review-audit-20261012.json';assert not out.exists();out.write_text(json.dumps(r,indent=2)+'\n');print(r['status'])
if __name__=='__main__':main()
