"""Independently verify retained current pixel/sequence test evidence."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-current-pixel-sequence-20261016.json';d=json.loads(P.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();W=Path(d['harness']).parent
for group in ['source_pins','dependency_pins','logs']:
 for p,sha in d[group].items():assert h(R/p if group=='source_pins' else p)==sha,(group,p)
for key in ['binary','harness','producer']:assert h(R/d[key] if key=='producer' else d[key])==d[key+'_sha256']
count=0
for m in ['pixels','linear','sequence']:
 for kind,prefix in [('src',''),('tests','test_')]:
  raw=(R/f'crates/rrrah-dedup/{kind}/{m}.rs').read_bytes();assert raw==(W/(prefix+m+'.rs')).read_bytes();count+=raw.count(b'#[test]')
s=(R/'crates/rrrah-dedup/src/local.rs').read_text();error=s[s.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):s.index('\n/// Extract deterministic')];helper=s[s.index('pub(crate) fn reserve_slot'):s.index('\n#[allow(clippy::too_many_lines)]')]
expected='extern crate self as rrrah_dedup;\nmod local {\n'+error+'\n'+helper+'\n}\n'+''.join(f'#[path="{m}.rs"]pub mod {m};\n#[path="test_{m}.rs"]mod {m}_controls;\n' for m in ['pixels','linear','sequence']);assert expected==(W/'harness.rs').read_text()
listing=subprocess.check_output([d['binary'],'--list'],text=True);names=re.findall(r'^(.*): test$',listing,re.M);assert names==d['inventory'] and len(names)==len(set(names))==count==21
log=(W/'tests.log').read_text();results=re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)$',log,re.M);assert sorted(results)==sorted((n,'ok') for n in names);assert '21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in log
out=D/'dedup-current-pixel-sequence-audit-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_complete_retained_current_pixel_linear_sequence_scope','passed':count,'report':str(P.relative_to(R)),'report_sha256':h(P),'auditor_sha256':h(__file__),'scope':d['scope']},indent=2)+'\n');print('Verified 21 tests, six whole unchanged source files, helper extraction, binary inventory, logs and dependency pins')
